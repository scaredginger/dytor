use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(UniquelyNamed)]
pub fn uniquely_named_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;
    let expanded = quote! {
        impl UniquelyNamed for #name {
            fn name() -> &'static str {
                stringify!(#name)
            }
        }
    };
    TokenStream::from(expanded)
}
