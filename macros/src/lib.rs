use proc_macro::TokenStream;
use syn::Error;

mod include_models;

#[proc_macro]
pub fn include_models(_: TokenStream) -> TokenStream {
    self::include_models::transform()
        .unwrap_or_else(Error::into_compile_error)
        .into()
}
