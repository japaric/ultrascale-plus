extern crate proc_macro;

use proc_macro::TokenStream;
use std::{
    collections::HashSet,
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use proc_macro2::Span;
use quote::quote;
use rand::{Rng, SeedableRng};
use syn::{
    parse, parse_macro_input, spanned::Spanned, Ident, Item, ItemFn, ItemStatic, ReturnType, Stmt,
    Type, Visibility,
};

static CALL_COUNT: AtomicUsize = AtomicUsize::new(0);

// NOTE copy-paste from cortex-r-rt-macros v0.1.3
#[proc_macro_attribute]
pub fn entry(args: TokenStream, input: TokenStream) -> TokenStream {
    let f = parse_macro_input!(input as ItemFn);

    // check the function signature
    let valid_signature = f.constness.is_none()
        && f.vis == Visibility::Inherited
        && f.abi.is_none()
        && f.decl.inputs.is_empty()
        && f.decl.generics.params.is_empty()
        && f.decl.generics.where_clause.is_none()
        && f.decl.variadic.is_none()
        && match f.decl.output {
            ReturnType::Default => false,
            ReturnType::Type(_, ref ty) => match **ty {
                Type::Never(_) => true,
                _ => false,
            },
        };

    if !valid_signature {
        return parse::Error::new(
            f.span(),
            "`#[entry]` function must have signature `[unsafe] fn() -> !`",
        )
        .to_compile_error()
        .into();
    }

    if !args.is_empty() {
        return parse::Error::new(Span::call_site(), "This attribute accepts no arguments")
            .to_compile_error()
            .into();
    }

    // XXX should we blacklist other attributes?
    let attrs = f.attrs;
    let unsafety = f.unsafety;
    let hash = random_ident();
    let (statics, stmts) = match extract_static_muts(f.block.stmts) {
        Err(e) => return e.to_compile_error().into(),
        Ok(x) => x,
    };

    let vars = statics
        .into_iter()
        .map(|var| {
            let attrs = var.attrs;
            let ident = var.ident;
            let ty = var.ty;
            let expr = var.expr;

            quote!(
                #[allow(non_snake_case)]
                let #ident: &'static mut #ty = unsafe {
                    #(#attrs)*
                    static mut #ident: #ty = #expr;

                    &mut #ident
                };
            )
        })
        .collect::<Vec<_>>();

    quote!(
        #[export_name = "main"]
        #[link_section = ".main"]
        #(#attrs)*
        #unsafety fn #hash() -> ! {
            #(#vars)*

            #(#stmts)*
        }
    )
    .into()
}

#[proc_macro_attribute]
pub fn exception(args: TokenStream, input: TokenStream) -> TokenStream {
    let f = parse_macro_input!(input as ItemFn);

    if !args.is_empty() {
        return parse::Error::new(Span::call_site(), "This attribute accepts no arguments")
            .to_compile_error()
            .into();
    }

    let fspan = f.span();
    let ident = f.ident;

    let ident_s = ident.to_string();

    // XXX should we blacklist other attributes?
    let attrs = f.attrs;
    let block = f.block;
    let stmts = block.stmts;
    let unsafety = f.unsafety;

    let hash = random_ident();
    let valid_signature = f.constness.is_none()
        && f.vis == Visibility::Inherited
        && f.abi.is_none()
        && f.decl.inputs.is_empty()
        && f.decl.generics.params.is_empty()
        && f.decl.generics.where_clause.is_none()
        && f.decl.variadic.is_none()
        && match f.decl.output {
            ReturnType::Default => false,
            ReturnType::Type(_, ref ty) => match **ty {
                Type::Never(..) => true,
                _ => false,
            },
        };

    if !valid_signature {
        return parse::Error::new(
            fspan,
            "This exception must have signature `[unsafe] fn() -> !`",
        )
        .to_compile_error()
        .into();
    }

    quote!(
        #[export_name = #ident_s]
        #(#attrs)*
        pub #unsafety extern "C" fn #hash() -> ! {
            extern crate zup_rt;

            // check that this exception actually exists
            zup_rt::Exception::#ident;

            #(#stmts)*
        }
    )
    .into()
}

#[proc_macro_attribute]
pub fn interrupt(args: TokenStream, input: TokenStream) -> TokenStream {
    let f = parse_macro_input!(input as ItemFn);

    if !args.is_empty() {
        return parse::Error::new(Span::call_site(), "This attribute accepts no arguments")
            .to_compile_error()
            .into();
    }

    let fspan = f.span();
    let ident = f.ident;

    let ident_s = ident.to_string();
    // XXX should we blacklist other attributes?
    let attrs = f.attrs;
    let block = f.block;
    let stmts = block.stmts;
    let unsafety = f.unsafety;

    let hash = random_ident();
    let valid_signature = f.constness.is_none()
        && f.vis == Visibility::Inherited
        && f.abi.is_none()
        && f.decl.inputs.is_empty()
        && f.decl.generics.params.is_empty()
        && f.decl.generics.where_clause.is_none()
        && f.decl.variadic.is_none()
        && match f.decl.output {
            ReturnType::Default => true,
            ReturnType::Type(_, ref ty) => match **ty {
                Type::Tuple(ref tuple) => tuple.elems.is_empty(),
                Type::Never(..) => true,
                _ => false,
            },
        };

    if !valid_signature {
        return parse::Error::new(
            fspan,
            "`#[exception]` handlers other than `DefaultHandler` and `Undefined` must have \
             signature `[unsafe] fn() [-> !]`",
        )
        .to_compile_error()
        .into();
    }

    let (statics, stmts) = match extract_static_muts(stmts) {
        Err(e) => return e.to_compile_error().into(),
        Ok(x) => x,
    };

    // FIXME this is not correct for the FIQ and maybe other exception handlers
    let vars = statics
        .into_iter()
        .map(|var| {
            let attrs = var.attrs;
            let ident = var.ident;
            let ty = var.ty;
            let expr = var.expr;

            quote!(
                #[allow(non_snake_case)]
                let #ident: &mut #ty = unsafe {
                    #(#attrs)*
                    static mut #ident: #ty = #expr;

                    &mut #ident
                };
            )
        })
        .collect::<Vec<_>>();

    quote!(
        #[export_name = #ident_s]
        #(#attrs)*
        pub #unsafety extern "C" fn #hash() {
            extern crate zup_rt;

            // check that this exception actually exists
            zup_rt::Interrupt::#ident;

            #(#vars)*

            #(#stmts)*
        }
    )
    .into()
}

// NOTE copy-paste from cortex-r-rt-macros v0.1.3
fn extract_static_muts(stmts: Vec<Stmt>) -> Result<(Vec<ItemStatic>, Vec<Stmt>), parse::Error> {
    let mut istmts = stmts.into_iter();

    let mut seen = HashSet::new();
    let mut statics = vec![];
    let mut stmts = vec![];
    while let Some(stmt) = istmts.next() {
        match stmt {
            Stmt::Item(Item::Static(var)) => {
                if var.mutability.is_some() {
                    if seen.contains(&var.ident) {
                        return Err(parse::Error::new(
                            var.ident.span(),
                            format!("the name `{}` is defined multiple times", var.ident),
                        ));
                    }

                    seen.insert(var.ident.clone());
                    statics.push(var);
                } else {
                    stmts.push(Stmt::Item(Item::Static(var)));
                }
            }
            _ => {
                stmts.push(stmt);
                break;
            }
        }
    }

    stmts.extend(istmts);

    Ok((statics, stmts))
}

fn random_ident() -> Ident {
    Ident::new(&random_string(), Span::call_site())
}

// NOTE copy-paste from cortex-r-rt-macros v0.1.3
fn random_string() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let count: u64 = CALL_COUNT.fetch_add(1, Ordering::SeqCst) as u64;
    let mut seed: [u8; 16] = [0; 16];

    for (i, v) in seed.iter_mut().take(8).enumerate() {
        *v = ((secs >> (i * 8)) & 0xFF) as u8
    }

    for (i, v) in seed.iter_mut().skip(8).enumerate() {
        *v = ((count >> (i * 8)) & 0xFF) as u8
    }

    let mut rng = rand::rngs::SmallRng::from_seed(seed);
    (0..16)
        .map(|i| {
            if i == 0 || rng.gen() {
                ('a' as u8 + rng.gen::<u8>() % 25) as char
            } else {
                ('0' as u8 + rng.gen::<u8>() % 10) as char
            }
        })
        .collect()
}
