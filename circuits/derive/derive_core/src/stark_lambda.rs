use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{
    braced, parenthesized, parse_macro_input, parse_quote, token, Expr, GenericParam, Generics,
    Ident, Path, ReturnType, Token, Type, TypeParamBound, Pat, PatIdent,
};

use crate::remove_gen_attr;

#[allow(dead_code)]
pub(crate) struct StarkLambdaInput {
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
    pub generics_comma_token: Option<Token![,]>,

    pub captures_paren_token: Option<token::Paren>,
    pub captures: Punctuated<Expr, Token![,]>,
    pub captures_colon_token: Option<Token![:]>,
    pub capture_tys_paren_token: Option<token::Paren>,
    pub capture_tys: Punctuated<Type, Token![,]>,
    pub captures_comma_token: Option<Token![,]>,

    pub or1_token: Token![|],
    pub capture_pat: Option<Pat>,
    pub capture_pat_comma_token: Option<Token![,]>,
    pub stark_id: Ident,
    pub stark_comma_token: Token![,],
    pub kind_id: Ident,
    pub kind_colon_token: Token![:],
    pub kind_ty: Type,
    pub or2_token: Token![|],
    pub output: ReturnType,
    pub body: Box<Expr>,
}

impl Parse for StarkLambdaInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let field = input.parse()?;
        let field_colon_token: Option<_> = input.parse()?;
        let mut field_bounds = Punctuated::new();
        if field_colon_token.is_some() {
            loop {
                if input.peek(Token![=]) || input.peek(Token![,]) {
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
        let generics_comma_token = if generics.lt_token.is_some() {
            if input.peek(Token![where]) {
                generics.where_clause = Some(input.parse()?);
                let _inner;
                let _: token::Brace = braced!(_inner in input);
            }
            Some(input.parse()?)
        } else {
            None
        };

        // Optionally parse captures
        let (captures_paren_token, captures, captures_colon_token, capture_tys_paren_token, capture_tys, captures_comma_token);
        if input.peek(token::Paren) {
            let paren_input;
            captures_paren_token = Some(parenthesized!(paren_input in input));
            captures = Punctuated::parse_terminated(&paren_input)?;
            captures_colon_token = Some(input.parse()?);
            let paren_input;
            capture_tys_paren_token = Some(parenthesized!(paren_input in input));
            capture_tys = Punctuated::parse_terminated(&paren_input)?;
            captures_comma_token = Some(input.parse()?);
        } else if !input.peek(Token![|]) {
            captures_paren_token = None;
            captures = Punctuated::from_iter([input.parse::<Expr>()?]);
            captures_colon_token = Some(input.parse()?);
            capture_tys_paren_token = None;
            capture_tys = Punctuated::from_iter([input.parse::<Type>()?]);
            captures_comma_token = Some(input.parse()?);
        } else {
            captures_paren_token = None;
            captures = Punctuated::new();
            captures_colon_token = None;
            capture_tys_paren_token = None;
            capture_tys = Punctuated::new();
            captures_comma_token = None;
        };

        let or1_token = input.parse()?;
        // Optionally parse capture id
        let (capture_pat, capture_pat_comma_token, stark_id, stark_comma_token) = if input.peek(Ident) && input.peek2(Token![,]) {
            let ident = input.parse()?;
            let comma_token = input.parse()?;
            if input.peek2(Token![:]) {
                (None, None, ident, comma_token)
            } else {
                let capture_pat = Pat::Ident(PatIdent{ident, attrs: vec![], by_ref: None, mutability: None, subpat: None});
                (Some(capture_pat), Some(comma_token), input.parse()?, input.parse()?)
            }
        } else {
            (Some(Pat::parse_single(input)?), Some(input.parse()?), input.parse()?, input.parse()?)
        };
        let kind_id = input.parse()?;
        let kind_colon_token = input.parse()?;
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
            generics_comma_token,

            captures_paren_token,
            captures,
            captures_colon_token,
            capture_tys_paren_token,
            capture_tys,
            captures_comma_token,

            or1_token,
            capture_pat,
            capture_pat_comma_token,
            stark_id,
            stark_comma_token,
            kind_id,
            kind_colon_token,
            kind_ty,
            or2_token,
            output,
            body,
        })
    }
}

pub fn stark_lambda(input: TokenStream, mutable: bool) -> TokenStream {
    let ast = parse_macro_input!(input as StarkLambdaInput);
    let crate_name: Path = parse_quote!(::mozak_circuits_derive);

    let trait_name = if mutable {
        Ident::new("StarkLambdaMut", Span::mixed_site())
    } else {
        Ident::new("StarkLambda", Span::mixed_site())
    };
    
    let mut_token = if mutable {
        Some(Token![mut](Span::mixed_site()))
    } else {
        None
    };

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
            bounded_field =
                quote!(#field: #crate_name::RichField + #crate_name::Extendable<#d_bare>,);
            unbounded_field = quote!(#field,);
            bare_field = quote!(#field);
            field_invoke = quote!(#field);
        }
        (Some(_), None) => {
            bounded_field = quote!(#field: #field_bounds,);
            unbounded_field = quote!(#field,);
            bare_field = quote!(#field);
            field_invoke = quote!(#field);
        }
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
        }
    }

    let generic_params = &ast.generics.params;
    let where_clause = &ast.generics.where_clause;

    let mut lifetimes: Punctuated<GenericParam, Token![,]> = generic_params
        .into_iter()
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
    let non_lifetimes: Punctuated<GenericParam, Token![,]> = generic_params
        .into_iter()
        .filter(|x| !matches!(x, GenericParam::Lifetime(_)))
        .cloned()
        .collect();
    let non_lifetimes_no_attr = remove_gen_attr(&non_lifetimes);

    let captures = &ast.captures;
    let capture_tys = &ast.capture_tys;

    let capture_pat = ast
        .capture_pat
        .map(|capture_pat| quote!(let #capture_pat = &mut self.captures;));
    let stark_id = &ast.stark_id;
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
        struct #trait_name<#lifetimes #bounded_field #bounded_d #non_lifetimes>
        #where_clause {
            _marker: core::marker::PhantomData<#bare_field>,
            captures: (#capture_tys),
        };
        impl<#lifetimes #bounded_field #bounded_d #non_lifetimes> #crate_name::#trait_name<#d_bare> for #trait_name<#lifetimes_no_attr #unbounded_field #unbounded_d #non_lifetimes_no_attr>
        #where_clause {
            type F = #bare_field;
            type Kind = #kind_ty;
            type Output = #output;
            fn call<S>(&mut self, #stark_id: &#mut_token S, #kind_id: Self::Kind) -> Self::Output
            where S: #crate_name::Stark<Self::F, #d_bare> {
                #capture_pat
                #body
            }
        }
        #trait_name{
            _marker: core::marker::PhantomData::<#field_invoke>,
            captures: (#captures),
        }
    }).into()
}
