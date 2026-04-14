use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, parse_macro_input};

#[proc_macro_derive(XdrEncode, attributes(xdr))]
pub fn derive_xdr_encode(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let body = match &input.data {
        Data::Struct(data) => encode_struct_fields(&data.fields),
        Data::Enum(_) => {
            return syn::Error::new_spanned(&input, "enums not yet supported")
                .to_compile_error()
                .into();
        }
        Data::Union(_) => {
            return syn::Error::new_spanned(&input, "unions not supported")
                .to_compile_error()
                .into();
        }
    };

    let expanded = quote! {
        impl #impl_generics xdr_codec::XdrEncode for #name #ty_generics #where_clause {
            fn encode(&self, buf: &mut bytes::BytesMut) -> Result<(), xdr_codec::XdrError> {
                #body
                Ok(())
            }
        }
    };

    expanded.into()
}

#[proc_macro_derive(XdrDecode, attributes(xdr))]
pub fn derive_xdr_decode(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let body = match &input.data {
        Data::Struct(data) => decode_struct_fields(name, &data.fields),
        Data::Enum(_) => {
            return syn::Error::new_spanned(&input, "enums not yet supported")
                .to_compile_error()
                .into();
        }
        Data::Union(_) => {
            return syn::Error::new_spanned(&input, "unions not supported")
                .to_compile_error()
                .into();
        }
    };

    let expanded = quote! {
        impl #impl_generics xdr_codec::XdrDecode for #name #ty_generics #where_clause {
            fn decode(buf: &mut bytes::Bytes) -> Result<Self, xdr_codec::XdrError> {
                #body
            }
        }
    };

    expanded.into()
}

fn encode_struct_fields(fields: &Fields) -> proc_macro2::TokenStream {
    match fields {
        Fields::Named(named) => {
            let encode_fields = named.named.iter().map(|f| {
                let name = &f.ident;
                quote! {
                    xdr_codec::XdrEncode::encode(&self.#name, buf)?;
                }
            });
            quote! { #(#encode_fields)* }
        }
        Fields::Unnamed(unnamed) => {
            let encode_fields = unnamed.unnamed.iter().enumerate().map(|(i, _)| {
                let idx = syn::Index::from(i);
                quote! {
                    xdr_codec::XdrEncode::encode(&self.#idx, buf)?;
                }
            });
            quote! { #(#encode_fields)* }
        }
        Fields::Unit => quote! {},
    }
}

fn decode_struct_fields(name: &syn::Ident, fields: &Fields) -> proc_macro2::TokenStream {
    match fields {
        Fields::Named(named) => {
            let decode_fields = named.named.iter().map(|f| {
                let field_name = &f.ident;
                quote! {
                    #field_name: xdr_codec::XdrDecode::decode(buf)?,
                }
            });
            quote! {
                Ok(#name {
                    #(#decode_fields)*
                })
            }
        }
        Fields::Unnamed(unnamed) => {
            let decode_fields = unnamed.unnamed.iter().map(|_| {
                quote! {
                    xdr_codec::XdrDecode::decode(buf)?,
                }
            });
            quote! {
                Ok(#name(
                    #(#decode_fields)*
                ))
            }
        }
        Fields::Unit => {
            quote! { Ok(#name) }
        }
    }
}
