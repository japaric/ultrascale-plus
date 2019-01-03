extern crate proc_macro;

use proc_macro::TokenStream;

use proc_macro2::Span;
use syn::{parse, parse_macro_input};

use crate::syntax::Amp;

mod codegen;
mod syntax;

#[proc_macro_attribute]
pub fn app(args: TokenStream, input: TokenStream) -> TokenStream {
    // Parse
    if !args.is_empty() {
        return parse::Error::new(Span::call_site(), "`#[amp]` takes no arguments")
            .to_compile_error()
            .into();
    }
    let items = parse_macro_input!(input as syntax::Input).items;
    let amp = match Amp::parse(items) {
        Err(e) => return e.to_compile_error().into(),
        Ok(amp) => amp,
    };

    codegen::amp(&amp)
}
