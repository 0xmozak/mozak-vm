use proc_macro::TokenStream;
use proc_macro_error::proc_macro_error;
use syn::punctuated::Punctuated;
use syn::{GenericParam, Token, TypeParam};

mod derive;
mod stark_kind_lambda;
mod stark_lambda;

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
    derive::derive_stark_display_name(input)
}

#[proc_macro_error]
#[proc_macro_derive(StarkSet, attributes(StarkSet))]
pub fn derive_stark_set(input: TokenStream) -> TokenStream { derive::derive_stark_set(input) }

#[proc_macro]
pub fn stark_kind_lambda(input: TokenStream) -> TokenStream {
    stark_kind_lambda::stark_kind_lambda(input)
}

#[proc_macro]
pub fn stark_lambda(input: TokenStream) -> TokenStream { stark_lambda::stark_lambda(input, false) }

#[proc_macro]
pub fn stark_lambda_mut(input: TokenStream) -> TokenStream {
    stark_lambda::stark_lambda(input, true)
}
