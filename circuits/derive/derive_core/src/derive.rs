use itertools::{multiunzip, Itertools};
use proc_macro::TokenStream;
use proc_macro2::{Literal, Span};
use proc_macro_error::{abort, emit_error};
use quote::{quote, ToTokens};
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{
    parse_macro_input, parse_quote, Data, DeriveInput, Expr, ExprLit, Ident, Index, Lit, Member,
    Meta, MetaNameValue, Path, Token,
};

use crate::remove_gen_attr;

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

pub fn derive_stark_set(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let ast_span = ast.span();

    let data = match ast.data {
        Data::Struct(data) => data,
        _ => abort!(ast, "only structs are supported"),
    };

    let crate_name: Path = parse_quote!(::mozak_circuits_derive);

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

    let enum_name = outer_attr
        .iter()
        .filter_map(|meta| {
            if meta.path.is_ident("enum_name") {
                Some(&meta.value)
            } else {
                None
            }
        })
        .at_most_one();
    let enum_name = match enum_name {
        Err(e) => {
            for enum_name in e {
                emit_error!(
                    enum_name,
                    "multiple 'enum_name' attributes for struct {:?}",
                    ast.ident
                );
            }
            None
        }
        Ok(enum_name) => enum_name,
    };
    let field = outer_attr
        .iter()
        .filter_map(|meta| {
            if meta.path.is_ident("field") {
                Some(&meta.value)
            } else {
                None
            }
        })
        .at_most_one();
    let field = match field {
        Err(e) => {
            for field in e {
                emit_error!(
                    field,
                    "multiple 'field' attributes for struct {:?}",
                    ast.ident
                );
            }
            None
        }
        Ok(field) => field,
    };
    let degree = outer_attr
        .iter()
        .filter_map(|meta| {
            if meta.path.is_ident("degree") {
                Some(&meta.value)
            } else {
                None
            }
        })
        .at_most_one();
    let degree = match degree {
        Err(e) => {
            for degree in e {
                emit_error!(
                    degree,
                    "multiple 'degree' attributes for struct {:?}",
                    ast.ident
                );
            }
            None
        }
        Ok(degree) => degree,
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
                    lit: Lit::Str(enum_name),
                    ..
                }) => Some((
                    field_id,
                    field_ty,
                    Ident::new(&enum_name.value(), Span::mixed_site()),
                )),
                kind => {
                    emit_error!(kind, "'stark_kind' should be a string literal");
                    None
                }
            }),
    );

    let enum_name = match enum_name {
        None => abort!(ast_span, "unique 'enum_name' is required"),
        Some(Expr::Lit(ExprLit {
            lit: Lit::Str(enum_name),
            ..
        })) => Ident::new(&enum_name.value(), Span::mixed_site()),
        Some(enum_name) => abort!(enum_name, "'enum_name' should be a string literal"),
    };
    let field = match field {
        None => abort!(ast_span, "unique 'field' is required"),
        Some(Expr::Lit(ExprLit {
            lit: Lit::Str(field),
            ..
        })) => Ident::new(&field.value(), Span::mixed_site()),
        Some(field) => abort!(field, "'field' should be a string literal"),
    };
    let degree = match degree {
        None => abort!(ast_span, "unique 'degree' is required"),
        Some(Expr::Lit(ExprLit {
            lit: Lit::Str(degree),
            ..
        })) => Ident::new(&degree.value(), Span::mixed_site()),
        Some(degree) => abort!(degree, "'degree' should be a string literal"),
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
            pub fn all_types<const D: usize, L>(l: L) -> [L::Output; Self::COUNT]
            where L: #crate_name::StarkKindLambda<D, Kind=Self>,
            L::F: #crate_name::RichField + #crate_name::Extendable<D>,
            {
                fn helper<F, const D: usize, L>(mut l: L) -> [L::Output; #enum_name::COUNT]
                where L: #crate_name::StarkKindLambda<D, F=F, Kind=#enum_name>,
                L::F: #crate_name::RichField + #crate_name::Extendable<D>,{
                    use #enum_name::*;
                    [#(l.call::<#field_tys>(#kinds),)*]
                }
                helper::<L::F, D, L>(l)
            }
        }

        /// Code generated via proc_macro `StarkSet`s
        impl<#generic_params> #ident<#generic_params_no_attr> {
            pub fn all_starks<L>(&self, mut l: L) -> [L::Output; #enum_name::COUNT]
            where L: #crate_name::StarkLambda<#degree, F=#field, Kind=#enum_name> {
                use #enum_name::*;
                [#(l.call::<#field_tys>(&self.#field_ids, #kinds),)*]
            }
            pub fn all_starks_mut<L>(&mut self, mut l: L) -> [L::Output; #enum_name::COUNT]
            where L: #crate_name::StarkLambdaMut<#degree, F=#field, Kind=#enum_name> {
                use #enum_name::*;
                [#(l.call::<#field_tys>(&mut self.#field_ids, #kinds),)*]
            }
        }
    )
    .into()
}
