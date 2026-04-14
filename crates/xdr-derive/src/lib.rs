use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Expr, Fields, Lit, Meta, parse_macro_input};

#[proc_macro_derive(XdrEncode, attributes(xdr))]
pub fn derive_xdr_encode(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let body = match &input.data {
        Data::Struct(data) => encode_struct_fields(&data.fields),
        Data::Enum(data) => encode_enum_variants(name, data),
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
        Data::Enum(data) => decode_enum_variants(name, data),
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

/// Extract the discriminant value for an enum variant.
///
/// Checks two sources in order:
/// 1. `#[xdr(discriminant = N)]` attribute on the variant
/// 2. Explicit Rust discriminant expression (e.g., `Ok = 0`)
fn get_discriminant(variant: &syn::Variant) -> Option<u32> {
    // Check for #[xdr(discriminant = N)] attribute
    for attr in &variant.attrs {
        if !attr.path().is_ident("xdr") {
            continue;
        }
        if let Meta::List(meta_list) = &attr.meta {
            let nested: syn::Result<Meta> = meta_list.parse_args();
            if let Ok(Meta::NameValue(nv)) = nested {
                if nv.path.is_ident("discriminant") {
                    if let Expr::Lit(expr_lit) = &nv.value {
                        if let Lit::Int(lit_int) = &expr_lit.lit {
                            return lit_int.base10_parse::<u32>().ok();
                        }
                    }
                }
            }
        }
    }

    // Fall back to explicit Rust discriminant (e.g., Ok = 0)
    if let Some((_, expr)) = &variant.discriminant {
        if let Expr::Lit(expr_lit) = expr {
            if let Lit::Int(lit_int) = &expr_lit.lit {
                return lit_int.base10_parse::<u32>().ok();
            }
        }
    }

    None
}

fn encode_enum_variants(
    name: &syn::Ident,
    data: &syn::DataEnum,
) -> proc_macro2::TokenStream {
    let arms = data.variants.iter().map(|variant| {
        let var_ident = &variant.ident;
        let disc = get_discriminant(variant)
            .unwrap_or_else(|| panic!("enum variant `{}` has no discriminant", var_ident));

        match &variant.fields {
            Fields::Unit => {
                quote! {
                    #name::#var_ident => {
                        xdr_codec::XdrEncode::encode(&(#disc as u32), buf)?;
                    }
                }
            }
            Fields::Unnamed(unnamed) => {
                let bindings: Vec<_> = (0..unnamed.unnamed.len())
                    .map(|i| syn::Ident::new(&format!("f{}", i), var_ident.span()))
                    .collect();
                let encode_fields = bindings.iter().map(|b| {
                    quote! { xdr_codec::XdrEncode::encode(#b, buf)?; }
                });
                quote! {
                    #name::#var_ident(#(#bindings),*) => {
                        xdr_codec::XdrEncode::encode(&(#disc as u32), buf)?;
                        #(#encode_fields)*
                    }
                }
            }
            Fields::Named(named) => {
                let field_idents: Vec<_> = named
                    .named
                    .iter()
                    .map(|f| f.ident.as_ref().unwrap())
                    .collect();
                let encode_fields = field_idents.iter().map(|f| {
                    quote! { xdr_codec::XdrEncode::encode(#f, buf)?; }
                });
                quote! {
                    #name::#var_ident { #(#field_idents),* } => {
                        xdr_codec::XdrEncode::encode(&(#disc as u32), buf)?;
                        #(#encode_fields)*
                    }
                }
            }
        }
    });

    quote! {
        match self {
            #(#arms)*
        }
    }
}

fn decode_enum_variants(
    name: &syn::Ident,
    data: &syn::DataEnum,
) -> proc_macro2::TokenStream {
    let type_name = name.to_string();

    let arms = data.variants.iter().map(|variant| {
        let var_ident = &variant.ident;
        let disc = get_discriminant(variant)
            .unwrap_or_else(|| panic!("enum variant `{}` has no discriminant", var_ident));

        let construct = match &variant.fields {
            Fields::Unit => {
                quote! { #name::#var_ident }
            }
            Fields::Unnamed(unnamed) => {
                let decode_fields = unnamed.unnamed.iter().map(|_| {
                    quote! { xdr_codec::XdrDecode::decode(buf)? }
                });
                quote! { #name::#var_ident(#(#decode_fields),*) }
            }
            Fields::Named(named) => {
                let field_decodes = named.named.iter().map(|f| {
                    let field_name = f.ident.as_ref().unwrap();
                    quote! { #field_name: xdr_codec::XdrDecode::decode(buf)? }
                });
                quote! { #name::#var_ident { #(#field_decodes),* } }
            }
        };

        quote! { #disc => Ok(#construct), }
    });

    quote! {
        let discriminant: u32 = xdr_codec::XdrDecode::decode(buf)?;
        match discriminant {
            #(#arms)*
            other => Err(xdr_codec::XdrError::InvalidEnum {
                discriminant: other,
                type_name: #type_name,
            }),
        }
    }
}
