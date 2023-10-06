use proc_macro::{self, TokenStream};
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(ColumnsView)]
pub fn derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let ident = input.ident;
    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let t = &generics.params[0];
    let size = quote! { std::mem::size_of::<#ident<u8>>() };

    let output = quote! {
        impl #impl_generics crate::columns_view::NumberOfColumns for #ident #ty_generics #where_clause {
            // `u8` is guaranteed to have a `size_of` of 1.
            const NUMBER_OF_COLUMNS: usize = #size;
        }

        // impl #impl_generics From<[#t; #size]> for #ident #ty_generics #where_clause {
        //     fn from(value: [#t; #size]) -> Self {
        //         unsafe { crate::columns_view::transmute_without_compile_time_size_checks(value) }
        //     }
        // }
        // impl #impl_generics From<[T; std::mem::size_of::<$s<u8>>()]> for $s<T> {
        //     fn from(value: [T; std::mem::size_of::<$s<u8>>()]) -> Self {
        //         unsafe { crate::columns_view::transmute_without_compile_time_size_checks(value) }
        //     }
        // }

    };
    proc_macro::TokenStream::from(output)
}
