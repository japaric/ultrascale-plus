#![allow(warnings)]
#![recursion_limit = "128"]

extern crate proc_macro;

use proc_macro::TokenStream;

use syn::{parse, parse_macro_input};

mod analyze;
mod check;
mod codegen;
mod syntax;

const PRIORITY_BITS: u8 = 5;

#[proc_macro_attribute]
pub fn app(args: TokenStream, input: TokenStream) -> TokenStream {
    // Parse
    let args = parse_macro_input!(args as syntax::AppArgs);
    let items = parse_macro_input!(input as syntax::Input).items;

    let app = match syntax::App::parse(items, args) {
        Err(e) => return e.to_compile_error().into(),
        Ok(app) => app,
    };

    // Check the specification
    if let Err(e) = check::app(&app) {
        return e.to_compile_error().into();
    }

    // Ceiling analysis
    let analysis = analyze::app(&app);

    // Code generation
    codegen::app(&app, &analysis)
}
