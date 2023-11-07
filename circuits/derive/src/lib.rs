extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::{parse_macro_input, DeriveInput, GenericParam, TypeParam};

#[proc_macro_derive(StarkNameDisplay)]
pub fn derive(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);

    let (ident, generic_params) = (ast.ident, ast.generics.params);

    // Converts `<F, const D: usize>` (sans `<` and `>`) to
    //          `<F, D>` (sans `<` and `>`)
    let generic_params_no_attr: Punctuated<GenericParam, Comma> = generic_params
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
