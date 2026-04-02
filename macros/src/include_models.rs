use proc_macro2::{Span, TokenStream};
use quote::quote;
use std::env::VarError;
use syn::Error;

pub fn transform() -> syn::Result<TokenStream> {
    let path = match std::env::var("PORM_GENERATED_FILE") {
        Ok(v) => v,
        Err(VarError::NotPresent) => {
            return Err(Error::new(
                Span::call_site(),
                "environment variable `PORM_GENERATED_FILE` not found",
            ));
        }
        Err(VarError::NotUnicode(_)) => {
            return Err(Error::new(
                Span::call_site(),
                "environment variable `PORM_GENERATED_FILE` is not unicode",
            ));
        }
    };

    Ok(quote! {
        ::std::include!(#path);
    })
}
