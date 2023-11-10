use itertools::{multiunzip, Itertools};
use proc_macro::TokenStream;
use proc_macro2::{Literal, Span};
use proc_macro_error::{abort, emit_error, proc_macro_error, abort_if_dirty};
use quote::{quote};
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{
    parse_macro_input, Data, DeriveInput, Expr, ExprLit, GenericParam, Ident, Index, Lit, Member,
    Meta, MetaNameValue, Token, TypeParam, Attribute,
};

/// Converts `<'a, F, const D: usize>` (sans `<` and `>`) to
///          `<'a, F, D>` (sans `<` and `>`)
fn remove_gen_attr(
    generic_params: &Punctuated<GenericParam, Token![,]>,
) -> Punctuated<GenericParam, Token![,]> {
    generic_params
        .iter()
        .map(|gen| match gen {
            GenericParam::Const(x) => GenericParam::Type(TypeParam {
                ident: x.ident.clone(),
                attrs: vec![],
                colon_token: None,
                bounds: Punctuated::new(),
                eq_token: None,
                default: None,
            }),
            GenericParam::Type(x) => GenericParam::Type(TypeParam {
                ident: x.ident.clone(),
                attrs: vec![],
                colon_token: None,
                bounds: Punctuated::new(),
                eq_token: None,
                default: None,
            }),
            x => x.clone(),
        })
        .collect()
}

#[proc_macro_derive(StarkNameDisplay)]
pub fn derive_stark_display_name(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);

    let ident = &ast.ident;
    let generic_params = &ast.generics.params;
    let where_clause = &ast.generics.where_clause;
    let generic_params_no_attr = remove_gen_attr(generic_params);

    quote!(
        /// Code generated via proc_macro `StarkNameDisplay`
        impl<#generic_params> std::fmt::Display for #ident<#generic_params_no_attr>
        #where_clause {
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
                emit_error!(
                    value,
                    "multiple '{}' attributes",
                    ident
                );
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

#[proc_macro_error]
#[proc_macro_derive(StarkSet, attributes(StarkSet))]
pub fn derive_stark_set(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);

    let data = match ast.data {
        Data::Struct(data) => data,
        _ => abort!(ast, "only structs are supported"),
    };

    let outer_attr = parse_attrs(ast.attrs, "StarkSet");
    let mod_name = get_attr(outer_attr, "mod_name");
    let mod_name = parse_attr(mod_name, "mod_name");

    let (field_ids, field_tys, kinds): (Vec<_>, Vec<_>, Vec<_>) = multiunzip(
        data.fields
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
                let field_attr = parse_attrs(field.attrs, "StarkSet");
                let kind = get_attr(field_attr, "stark_kind");
                let kind = parse_attr(kind, "stark_kind");
                kind.map(|kind| (
                    ident,
                    field.ty,
                    kind,
                ))
            })
    );

    let kind_count = Literal::usize_unsuffixed(kinds.len());
    let kinds_decl = kinds.iter().enumerate().map(|(i, kind)| {
        let i = Literal::usize_unsuffixed(i);
        quote!(#kind = #i,)
    });

    let ident = &ast.ident;
    let generic_params = &ast.generics.params;
    let generic_params_no_attr = remove_gen_attr(generic_params);
    
    abort_if_dirty();

    let result = quote!(
        mod #mod_name{
            use super::*;

            pub trait StarkKinds<#generic_params> {
                #(type #kinds;)*
            }
            impl<#generic_params> StarkKinds<#generic_params_no_attr> for #ident<#generic_params_no_attr> {
                #(type #kinds = #field_tys;)*
            }

            #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
            pub enum Kind {
                #(#kinds_decl)*
            }

            #[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash)]
            pub struct Builder<T> {
                #(pub #field_ids: T,)*
            }

            impl<T> Builder<T> {
                pub fn build(self) -> [T; Kind::COUNT] {
                    [#(self.#field_ids,)*]
                }
            }

            /// Code generated via proc_macro `StarkSet`
            impl Kind {
                pub const COUNT: usize = #kind_count;

                #[must_use]
                pub fn all() -> [Self; Self::COUNT] {
                    use Kind::*;
                    [#(#kinds,)*]
                }
            }

            macro_rules! all_kind {
                ($stark_ty:ty, $kind_ty:ty, $sk_ty:ty, |$stark:ident, $kind:ident| $val:expr) => {{
                    use $kind_ty::*;
                    [#(
                        {
                            macro_rules! $stark {
                                () => {<$stark_ty as $sk_ty>::#kinds}
                            }
                            let $kind = #kinds;
                            $val
                        },)*
                    ]
                }};
                ($kind_ty:ty, |$kind:ident| $val:expr) => {{
                    use $kind_ty::*;
                    [#(
                        {
                            let $kind = #kinds;
                            $val
                        },)*
                    ]
                }};
            }
            pub(crate) use all_kind;

            macro_rules! all_starks {
                () => {};
                ($all_stark:expr, $kind_ty:ty, |$stark:ident, $kind:ident| $val:expr) => {{
                    use core::borrow::Borrow;
                    use $kind_ty::*;
                    let all_stark = $all_stark.borrow();
                    [#(
                        {
                            let $stark = &all_stark.#field_ids;
                            let $kind = #kinds;
                            $val
                        },)*
                    ]
                }};
                ($all_stark:expr, $kind_ty:ty, |mut $stark:ident, $kind:ident| $val:expr) => {{
                    use core::borrow::BorrowMut;
                    use $kind_ty::*;
                    let all_stark = $all_stark.borrow_mut();
                    [#(
                        {
                            let $stark = &mut all_stark.#field_ids;
                            let $kind = #kinds;
                            $val
                        },)*
                    ]
                }};
            }
            pub(crate) use all_starks;
        }

    );

    result.into()
}
