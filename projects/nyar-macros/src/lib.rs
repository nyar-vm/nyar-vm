extern crate proc_macro;

#![warn(missing_docs)]

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields};

#[proc_macro_derive(Trace)]
pub fn derive_trace(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let trace_body = match input.data {
        Data::Struct(data) => match data.fields {
            Fields::Named(fields) => {
                let recurse = fields.named.iter().map(|f| {
                    let name = &f.ident;
                    quote! { self.#name.trace(ctx); }
                });
                quote! { #( #recurse )* }
            }
            Fields::Unnamed(fields) => {
                let recurse = fields.unnamed.iter().enumerate().map(|(i, _)| {
                    let index = syn::Index::from(i);
                    quote! { self.#index.trace(ctx); }
                });
                quote! { #( #recurse )* }
            }
            Fields::Unit => quote! {},
        },
        Data::Enum(data) => {
            let variants = data.variants.iter().map(|v| {
                let ident = &v.ident;
                match &v.fields {
                    Fields::Named(fields) => {
                        let names = fields.named.iter().map(|f| &f.ident);
                        let recurse = fields.named.iter().map(|f| {
                            let name = &f.ident;
                            quote! { #name.trace(ctx); }
                        });
                        quote! {
                            Self::#ident { #( #names ),* } => {
                                #( #recurse )*
                            }
                        }
                    }
                    Fields::Unnamed(fields) => {
                        let names = (0..fields.unnamed.len()).map(|i| {
                            syn::Ident::new(&format!("field{}", i), proc_macro2::Span::call_site())
                        });
                        let recurse = names.clone().map(|name| {
                            quote! { #name.trace(ctx); }
                        });
                        quote! {
                            Self::#ident( #( #names ),* ) => {
                                #( #recurse )*
                            }
                        }
                    }
                    Fields::Unit => {
                        quote! { Self::#ident => {} }
                    }
                }
            });
            quote! {
                match self {
                    #( #variants )*
                }
            }
        }
        Data::Union(_) => panic!("Trace cannot be derived for unions"),
    };

    let expanded = quote! {
        impl #impl_generics crate::Trace for #name #ty_generics #where_clause {
            fn trace(&self, ctx: &mut crate::MarkContext) {
                #trace_body
            }
        }
    };

    TokenStream::from(expanded)
}
