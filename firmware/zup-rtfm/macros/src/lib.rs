#![allow(warnings)]
// #![deny(warnings)]
#![recursion_limit = "128"]

extern crate proc_macro;

use proc_macro::TokenStream;
use std::{fs, path::Path};

use syn::parse_macro_input;

mod analyze;
mod check;
mod codegen;
mod syntax;

/* Device specific constants */

/// Number of supported priority levels
const PRIORITY_BITS: u8 = 5;

/// Number of SGIs provided by the hardware
const NSGIS: u8 = 16;

#[proc_macro_attribute]
pub fn app(args: TokenStream, input: TokenStream) -> TokenStream {
    // Parse
    let args = parse_macro_input!(args as syntax::AppArgs);
    let input = parse_macro_input!(input as syntax::Input);

    let app = match syntax::App::parse(input.items, args) {
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
    let expansion = codegen::app(&input.ident, &app, &analysis);

    if Path::new("target").exists() {
        fs::write("target/rtfm-expansion.rs", expansion.to_string()).ok();
    }

    expansion
}
