extern crate proc_macro;

use itertools::Itertools;
use proc_macro::TokenStream;
use proc_macro2::{Literal, Span};
use proc_macro_error::{abort, emit_error, proc_macro_error};
use quote::{quote, ToTokens};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{token, braced};
use syn::{
    parenthesized, parse_macro_input, parse_quote, Data, DeriveInput, Expr, ExprLit, GenericParam,
    Generics, Ident, Index, Lit, Member, Meta, Path, ReturnType, Token, Type, TypeParam, TypeParamBound,
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

#[proc_macro_error]
#[proc_macro_derive(StarkSet, attributes(stark_enum, stark_kind))]
pub fn derive_stark_set(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);

    let data = match &ast.data {
        Data::Struct(data) => data,
        _ => abort!(ast, "only structs are supported"),
    };

    let crate_name: Path = parse_quote!(::mozak_circuits_derive);

    let enum_name = ast
        .attrs
        .iter()
        .filter_map(|attr| match &attr.meta {
            Meta::NameValue(meta) if meta.path.is_ident("stark_enum") => Some(&meta.value),
            _ => None,
        })
        .at_most_one();
    let enum_name = match enum_name {
        Err(e) => {
            for enum_name in e {
                emit_error!(
                    enum_name,
                    "multiple definitions of 'stark_enum' for struct {:?}",
                    ast.ident
                );
            }
            None
        }
        Ok(enum_name) => enum_name,
    };

    let (fields, kinds): (Vec<_>, Vec<_>) = data
        .fields
        .iter()
        .enumerate()
        .filter_map(|(index, field)| {
            let ident = match field.ident.as_ref() {
                Some(ident) => Member::Named(ident.clone()),
                None => Member::Unnamed(Index {
                    index: index as u32,
                    span: Span::mixed_site(),
                }),
            };
            let kind = field
                .attrs
                .iter()
                .filter_map(|attr| match &attr.meta {
                    Meta::NameValue(meta) if meta.path.is_ident("stark_kind") => Some(&meta.value),
                    _ => None,
                })
                .at_most_one();

            match kind {
                Err(e) => {
                    for kind in e {
                        emit_error!(
                            kind,
                            "multiple definitions of 'stark_kind' for field {}",
                            ident.to_token_stream()
                        );
                    }
                    None
                }
                Ok(kind) => kind.map(|kind| ((ident, &field.ty), kind)),
            }
        })
        .filter_map(|(field, kind)| match kind {
            Expr::Lit(ExprLit {
                lit: Lit::Str(enum_name),
                ..
            }) => Some((field, Ident::new(&enum_name.value(), Span::mixed_site()))),
            kind => {
                emit_error!(kind, "'stark_kind' should be a string literal");
                None
            }
        })
        .unzip();
    let (field_ids, field_tys): (Vec<_>, Vec<_>) = fields.into_iter().unzip();

    let enum_name = match enum_name {
        None => abort!(ast, "unique 'enum_name' is required"),
        Some(Expr::Lit(ExprLit {
            lit: Lit::Str(enum_name),
            ..
        })) => Ident::new(&enum_name.value(), Span::mixed_site()),
        Some(enum_name) => abort!(enum_name, "'enum_name' should be a string literal"),
    };

    let kind_count = Literal::usize_unsuffixed(kinds.len());
    let kinds_decl = kinds.iter().enumerate().map(|(i, kind)| {
        let i = Literal::usize_unsuffixed(i);
        quote!(#kind = #i,)
    });

    let (ident, generic_params) = (ast.ident, ast.generics.params);
    let generic_params_no_attr = remove_gen_attr(&generic_params);

    quote!(
        #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
        pub enum #enum_name {
            #(#kinds_decl)*
        }

        /// Code generated via proc_macro `StarkSet`
        impl #enum_name {
            pub(crate) const COUNT: usize = #kind_count;

            #[must_use]
            pub fn all() -> [Self; Self::COUNT] {
                use #enum_name::*;
                [#(#kinds,)*]
            }
            pub fn all_types<const D: usize, L>(mut l: L) -> [L::Output; Self::COUNT]
            where L: #crate_name::StarkKindFnMut<D, Kind=Self>,
            L::F: #crate_name::RichField + #crate_name::Extendable<D>,
            {
                fn helper<F, const D: usize, L>(mut l: L) -> [L::Output; #enum_name::COUNT]
                where L: #crate_name::StarkKindFnMut<D, F=F, Kind=#enum_name>,
                L::F: #crate_name::RichField + #crate_name::Extendable<D>,{
                    use #enum_name::*;
                    [#(l.call::<#field_tys>(#kinds),)*]
                }
                helper::<L::F, D, L>(l)
            }
        }

        /// Code generated via proc_macro `StarkSet`s
        impl<#generic_params> #ident<#generic_params_no_attr> {
            pub(crate) fn nums_permutation_zs(&self, config: &StarkConfig) -> [usize; #enum_name::COUNT] {
                [#(self.#field_ids.num_permutation_batches(config),)*]
            }
            pub(crate) fn permutation_batch_sizes(&self) -> [usize; #enum_name::COUNT] {
                [#(self.#field_ids.permutation_batch_size(),)*]
            }
        }
    ).into()
}

#[allow(dead_code)]
pub(crate) struct LambdaInput {
    pub field: Ident,
    pub field_colon_token: Option<Token![:]>,
    pub field_bounds: Punctuated<TypeParamBound, Token![+]>,
    pub field_eq_token: Option<Token![=]>,
    pub field_ty: Option<Type>,
    pub field_comma_token: Token![,],

    pub d: Ident,
    pub d_eq_token: Option<Token![=]>,
    pub d_val: Option<Expr>,
    pub d_comma_token: Token![,],

    pub generics: Generics,
    pub comma_3_token: Option<Token![,]>,

    pub paren_token_1: Option<token::Paren>,
    pub captures: Punctuated<Expr, Token![,]>,
    pub colon_token_1: Option<Token![:]>,
    pub paren_token_2: Option<token::Paren>,
    pub capture_tys: Punctuated<Type, Token![,]>,
    pub comma_4_token: Option<Token![,]>,

    pub or1_token: Token![|],
    pub capture_id: Option<Ident>,
    pub comma_5_token: Option<Token![,]>,
    pub kind_id: Ident,
    pub colon_token_2: Token![:],
    pub kind_ty: Type,
    pub or2_token: Token![|],
    pub output: ReturnType,
    pub body: Box<Expr>,
}

impl Parse for LambdaInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let field = input.parse()?;
        let field_colon_token: Option<_> = input.parse()?;
        let mut field_bounds = Punctuated::new();
        if field_colon_token.is_some() {
            loop {
                if input.peek(Token![=]) || input.peek(Token![,])  {
                    break;
                }
                let value: TypeParamBound = input.parse()?;
                field_bounds.push_value(value);
                if !input.peek(Token![+]) {
                    break;
                }
                let punct: Token![+] = input.parse()?;
                field_bounds.push_punct(punct);
            }
        }
        let field_eq_token: Option<_> = input.parse()?;
        let field_ty = if field_eq_token.is_some() {
            Some(input.parse()?)
        } else {
            None
        };
        let field_comma_token = input.parse()?;

        let d = input.parse()?;
        let d_eq_token: Option<_> = input.parse()?;
        let d_val = if d_eq_token.is_some() {
            Some(input.parse()?)
        } else {
            None
        };
        let d_comma_token = input.parse()?;

        // Optionally parse generics
        let mut generics = input.parse::<Generics>()?;
        let comma_3_token = if generics.lt_token.is_some() {
            if input.peek(Token![where]) {
                generics.where_clause = Some(input.parse()?);
            }
            let _inner;
            let _: token::Brace = braced!(_inner in input);
            Some(input.parse()?)
        } else {
            None
        };

        // Optionally parse captures
        let (paren_token_1, captures, colon_token_1, paren_token_2, capture_tys, comma_4_token);
        if input.peek(token::Paren) {
            let paren_input;
            paren_token_1 = Some(parenthesized!(paren_input in input));
            captures = Punctuated::parse_terminated(&paren_input)?;
            colon_token_1 = Some(input.parse()?);
            let paren_input;
            paren_token_2 = Some(parenthesized!(paren_input in input));
            capture_tys = Punctuated::parse_terminated(&paren_input)?;
            comma_4_token = Some(input.parse()?);
        } else if !input.peek(Token![|]) {
            paren_token_1 = None;
            captures = Punctuated::from_iter([input.parse::<Expr>()?]);
            colon_token_1 = Some(input.parse()?);
            paren_token_2 = None;
            capture_tys = Punctuated::from_iter([input.parse::<Type>()?]);
            comma_4_token = Some(input.parse()?);
        } else {
            paren_token_1 = None;
            captures = Punctuated::new();
            colon_token_1 = None;
            paren_token_2 = None;
            capture_tys = Punctuated::new();
            comma_4_token = None;
        };

        let or1_token = input.parse()?;
        let id: Ident = input.parse()?;
        // Optionally parse capture id
        let (capture_id, comma_5_token, kind_id) = if input.peek(Token![:]) {
            (None, None, id)
        } else {
            (Some(id), input.parse()?, input.parse()?)
        };
        let colon_token_2 = input.parse()?;
        let kind_ty = input.parse()?;
        let or2_token = input.parse()?;

        let output = input.parse()?;
        let body = input.parse()?;

        Ok(Self {
            field,
            field_colon_token,
            field_bounds,
            field_eq_token,
            field_ty,
            field_comma_token,

            d,
            d_eq_token,
            d_val,
            d_comma_token,

            generics,
            comma_3_token,

            paren_token_1,
            captures,
            colon_token_1,
            paren_token_2,
            capture_tys,
            comma_4_token,

            or1_token,
            capture_id,
            comma_5_token,
            kind_id,
            colon_token_2,
            kind_ty,
            or2_token,
            output,
            body,
        })
    }
}

#[proc_macro]
pub fn stark_kind_lambda(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as LambdaInput);

    let crate_name: Path = parse_quote!(::mozak_circuits_derive);

    let (bounded_d, unbounded_d, d_bare);
    if let Some(d_val) = ast.d_val.as_ref() {
        bounded_d = quote!();
        unbounded_d = quote!();
        d_bare = quote!(#d_val);
    } else {
        let d = Ident::new(&ast.d.to_string(), Span::mixed_site());
        bounded_d = quote!(const #d: usize,);
        unbounded_d = quote!(#d,);
        d_bare = quote!(#d);
    }


    let field = &ast.field;
    let field_bounds = &ast.field_bounds;
    let (bounded_field, unbounded_field, bare_field, field_invoke);
    match (ast.field_colon_token.as_ref(), ast.field_ty.as_ref()) {
        (None, None) => {
            bounded_field = quote!(#field: #crate_name::RichField + #crate_name::Extendable<#d_bare>,);
            unbounded_field = quote!(#field,);
            bare_field = quote!(#field);
            field_invoke = quote!(#field);
        },
        (Some(_), None) => {
            bounded_field = quote!(#field: #field_bounds,);
            unbounded_field = quote!(#field,);
            bare_field = quote!(#field);
            field_invoke = quote!(#field);
        },
        (None, Some(field_ty)) => {
            bounded_field = quote!();
            unbounded_field = quote!();
            bare_field = quote!(#field_ty);
            field_invoke = quote!(#field_ty);
        }
        (Some(_), Some(field_ty)) => {
            bounded_field = quote!(#field: #field_bounds,);
            unbounded_field = quote!(#field,);
            bare_field = quote!(#field);
            field_invoke = quote!(#field_ty);
        },
    }

    let generic_params = &ast.generics.params;
    let where_clause = &ast.generics.where_clause;

    let mut lifetimes: Punctuated<GenericParam, Token![,]> = generic_params.into_iter()
        .filter(|x| matches!(x, GenericParam::Lifetime(_)))
        .cloned()
        .collect();
    if !lifetimes.empty_or_trailing() {
        lifetimes.push_punct(Default::default());
    }
    let mut lifetimes_no_attr = remove_gen_attr(&lifetimes);
    if !lifetimes_no_attr.empty_or_trailing() {
        lifetimes_no_attr.push_punct(Default::default());
    }
    let non_lifetimes: Punctuated<GenericParam, Token![,]> = generic_params.into_iter()
        .filter(|x| !matches!(x, GenericParam::Lifetime(_)))
        .cloned()
        .collect();
    let non_lifetimes_no_attr = remove_gen_attr(&non_lifetimes);

    let captures = &ast.captures;
    let capture_tys = &ast.capture_tys;

    let capture_id = ast
        .capture_id
        .map(|capture_id| quote!(let #capture_id = &mut self.captures;));
    let kind_id = &ast.kind_id;
    let kind_ty = &ast.kind_ty;
    let output_storage;
    let output = match &ast.output {
        ReturnType::Default => {
            output_storage = Box::new(parse_quote!(()));
            &output_storage
        }
        ReturnType::Type(_, ty) => ty,
    };
    let body = &ast.body;

    quote!({
        struct StarkKindFnMut<#lifetimes #bounded_field #bounded_d #non_lifetimes>
        #where_clause {
            _marker: core::marker::PhantomData<#bare_field>,
            captures: (#capture_tys),
        };
        impl<#lifetimes #bounded_field #bounded_d #non_lifetimes> #crate_name::StarkKindFnMut<#d_bare> for StarkKindFnMut<#lifetimes_no_attr #unbounded_field #unbounded_d #non_lifetimes_no_attr>
        #where_clause {
            type F = #bare_field;
            type Kind = #kind_ty;
            type Output = #output;
            fn call<S>(&mut self, #kind_id: Self::Kind) -> Self::Output
            where S: #crate_name::Stark<Self::F, #d_bare> {
                #capture_id
                #body
            }
        }
        StarkKindFnMut{
            _marker: core::marker::PhantomData::<#field_invoke>,
            captures: (#captures),
        }
    }).into()
}
