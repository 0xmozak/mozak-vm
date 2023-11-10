use itertools::{multiunzip, Itertools};
use proc_macro::TokenStream;
use proc_macro2::{Literal, Span};
use proc_macro_error::{abort, emit_error, proc_macro_error};
use quote::{quote, ToTokens};
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{
    parse_macro_input, Data, DeriveInput, Expr, ExprLit, GenericParam, Ident, Index, Lit, Member,
    Meta, MetaNameValue, Token, TypeParam,
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
#[proc_macro_derive(StarkSet, attributes(StarkSet))]
pub fn derive_stark_set(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let ast_span = ast.span();

    let data = match ast.data {
        Data::Struct(data) => data,
        _ => abort!(ast, "only structs are supported"),
    };

    let outer_attr = ast
        .attrs
        .into_iter()
        .filter_map(|attr| match attr.meta {
            Meta::List(meta) if meta.path.is_ident("StarkSet") => Some(meta.tokens),
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
        .collect::<Vec<MetaNameValue>>();

    let mod_name = outer_attr
        .iter()
        .filter_map(|meta| {
            if meta.path.is_ident("mod_name") {
                Some(&meta.value)
            } else {
                None
            }
        })
        .at_most_one();
    let mod_name = match mod_name {
        Err(e) => {
            for mod_name in e {
                emit_error!(
                    mod_name,
                    "multiple 'mod_name' attributes for struct {:?}",
                    ast.ident
                );
            }
            None
        }
        Ok(mod_name) => mod_name,
    };

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
                let attr = field
                    .attrs
                    .into_iter()
                    .filter_map(|attr| match attr.meta {
                        Meta::List(meta) if meta.path.is_ident("StarkSet") => Some(meta.tokens),
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
                    .collect::<Vec<MetaNameValue>>();

                let kind = attr
                    .into_iter()
                    .filter_map(|meta| {
                        if meta.path.is_ident("stark_kind") {
                            Some(meta.value)
                        } else {
                            None
                        }
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
                    Ok(kind) => kind.map(|kind| (ident, field.ty, kind)),
                }
            })
            .filter_map(|(field_id, field_ty, kind)| match kind {
                Expr::Lit(ExprLit {
                    lit: Lit::Str(mod_name),
                    ..
                }) => Some((
                    field_id,
                    field_ty,
                    Ident::new(&mod_name.value(), Span::mixed_site()),
                )),
                kind => {
                    emit_error!(kind, "'stark_kind' should be a string literal");
                    None
                }
            }),
    );

    let mod_name = match mod_name {
        None => abort!(ast_span, "unique 'mod_name' is required"),
        Some(Expr::Lit(ExprLit {
            lit: Lit::Str(mod_name),
            ..
        })) => Ident::new(&mod_name.value(), Span::mixed_site()),
        Some(mod_name) => abort!(mod_name, "'mod_name' should be a string literal"),
    };

    let kind_count = Literal::usize_unsuffixed(kinds.len());
    let kinds_decl = kinds.iter().enumerate().map(|(i, kind)| {
        let i = Literal::usize_unsuffixed(i);
        quote!(#kind = #i,)
    });

    let ident = &ast.ident;
    let generic_params = &ast.generics.params;
    let generic_params_no_attr = remove_gen_attr(generic_params);

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
