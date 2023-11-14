use itertools::{multiunzip, Itertools};
use proc_macro::TokenStream;
use proc_macro2::{Literal, Span};
use proc_macro_error::{abort, abort_if_dirty, emit_error, emit_warning, proc_macro_error};
use quote::quote;
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{
    parse_macro_input, Attribute, Data, DeriveInput, Expr, ExprLit, GenericParam, Ident, Index,
    Lit, Member, Meta, MetaNameValue, Token, TypeParam,
};

#[proc_macro_derive(StarkNameDisplay)]
pub fn derive(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);

    let (ident, generic_params) = (ast.ident, ast.generics.params);

    // Converts `<F, const D: usize>` (sans `<` and `>`) to
    //          `<F, D>` (sans `<` and `>`)
    let generic_params_no_attr: Punctuated<GenericParam, Token![,]> = generic_params
        .iter()
        .map(|gen| match gen {
            GenericParam::Type(x) => GenericParam::Type(x.clone()),
            GenericParam::Const(x) => GenericParam::Type(TypeParam {
                ident: x.ident.clone(),
                attrs: vec![],
                colon_token: None,
                bounds: Punctuated::new(),
                eq_token: None,
                default: None,
            }),
            _ => unimplemented!(), // we don't expect lifetime annotations
        })
        .collect();

    quote!(
        /// Code generated via proc_macro `StarkNameDisplay`
        impl<#generic_params> std::fmt::Display for #ident<#generic_params_no_attr> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", std::stringify!(#ident))
            }
        }
    )
    .into()
}

fn parse_attrs(attrs: Vec<Attribute>, ident: &str) -> impl Iterator<Item = MetaNameValue> + '_ {
    attrs
        .into_iter()
        .filter_map(|attr| match attr.meta {
            Meta::List(meta) if meta.path.is_ident(ident) => Some(meta.tokens),
            _ => None,
        })
        .filter_map(|tokens| {
            let span = tokens.span();
            let parser = Punctuated::<MetaNameValue, Token![,]>::parse_terminated;
            match parser.parse2(tokens) {
                Ok(v) => Some(v),
                Err(e) => {
                    emit_error!(span, "failed to parse {}", e);
                    None
                }
            }
        })
        .flatten()
}

fn get_attr(attrs: impl Iterator<Item = MetaNameValue>, ident: &str) -> Option<Expr> {
    let value = attrs
        .into_iter()
        .filter_map(|meta| {
            if meta.path.is_ident(ident) {
                Some(meta.value)
            } else {
                None
            }
        })
        .at_most_one();
    match value {
        Err(e) => {
            for value in e {
                emit_error!(value, "multiple '{}' attributes", ident);
            }
            None
        }
        Ok(value) => value,
    }
}

fn parse_attr(attr: Option<Expr>, ident: &str) -> Option<Ident> {
    match attr {
        None => None,
        Some(Expr::Lit(ExprLit {
            lit: Lit::Str(attr),
            ..
        })) => Some(Ident::new(&attr.value(), Span::mixed_site())),
        Some(kind) => {
            emit_error!(kind, "'{}' should be a string literal", ident);
            None
        }
    }
}

fn parse_single_attr(attrs: Vec<Attribute>, attr_name: &str, key: &str) -> Option<Ident> {
    let attr = parse_attrs(attrs, attr_name);
    let val = get_attr(attr, key);
    parse_attr(val, key)
}

/// A derive macro which extracts metadata about a `struct` and embeds it in a
/// `macro`.
///
/// The resulting macro can be used with `tt_call` to easily generate custom
/// code with `macro_rules`.
#[proc_macro_error]
#[proc_macro_derive(StarkSet, attributes(StarkSet))]
pub fn derive_stark_set(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let ast_span = ast.span();

    let data = match ast.data {
        Data::Struct(data) => data,
        _ => abort!(ast, "only structs are supported"),
    };

    let macro_name = parse_single_attr(ast.attrs, "StarkSet", "macro_name")
        .unwrap_or_else(|| Ident::new("stark_set", Span::mixed_site()));

    let field_info = data
        .fields
        .into_iter()
        .enumerate()
        .filter_map(|(index, field)| {
            let ident = match field.ident {
                Some(ident) => Member::Named(ident.clone()),
                None => Member::Unnamed(Index {
                    index: index as u32,
                    span: Span::mixed_site(),
                }),
            };
            let kind = parse_single_attr(field.attrs, "StarkSet", "stark_kind");
            kind.map(|kind| (ident, field.ty, kind))
        });
    let (field_ids, field_tys, kinds): (Vec<_>, Vec<_>, Vec<_>) = multiunzip(field_info);

    if kinds.is_empty() {
        emit_warning!(
            ast_span,
            r#"No starks found, did you forget to tag fields with `#[StarkSet(stark_kind = "...")]`?"#
        );
    }
    let kind_count = Literal::usize_unsuffixed(kinds.len());
    let kind_vals = kinds
        .iter()
        .enumerate()
        .map(|(i, _)| Literal::usize_unsuffixed(i));

    abort_if_dirty();

    // Generate the macro
    let result = quote!(
        /// Code generated via proc_macro `StarkSet`
        macro_rules! #macro_name {
            {$caller:tt} => {
                tt_call::tt_return! {
                    $caller
                    kind_names = [{ #(#kinds)* }]
                    kind_vals = [{ #(#kind_vals)* }]
                    count = [{ #kind_count }]
                    tys = [{ #(#field_tys)* }]
                    fields = [{ #(#field_ids)* }]
                }
            };
        }
    );

    result.into()
}
