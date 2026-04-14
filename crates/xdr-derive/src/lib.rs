use proc_macro::TokenStream;

#[proc_macro_derive(XdrEncode, attributes(xdr))]
pub fn derive_xdr_encode(_input: TokenStream) -> TokenStream {
    TokenStream::new()
}

#[proc_macro_derive(XdrDecode, attributes(xdr))]
pub fn derive_xdr_decode(_input: TokenStream) -> TokenStream {
    TokenStream::new()
}
