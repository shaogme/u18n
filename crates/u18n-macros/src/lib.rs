mod dispatch;
mod invocation;

use proc_macro::TokenStream;

use crate::{dispatch::TranslationDispatcher, invocation::InvocationExpansion};

#[proc_macro]
pub fn invoke_message(input: TokenStream) -> TokenStream {
    match InvocationExpansion::expand(input.into()) {
        Ok(expansion) => expansion.into(),
        Err(error) => error.into_compile_error().into(),
    }
}

#[proc_macro]
pub fn dispatch_translation(input: TokenStream) -> TokenStream {
    match TranslationDispatcher::expand(input.into()) {
        Ok(expansion) => expansion.into(),
        Err(error) => error.into_compile_error().into(),
    }
}
