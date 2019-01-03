use proc_macro::TokenStream;

use quote::quote;

use crate::syntax::Amp;

pub fn amp(amp: &Amp) -> TokenStream {
    let mut items = vec![];

    for def in &amp.defs {
        items.push(quote!(#def));
    }

    for item in &amp.items {
        items.push(quote!(#[cfg(amp_shared)] #item));
    }

    for (ident, static_) in &amp.shared {
        let mut_ = &static_.mutability;
        let ty = &static_.ty;
        let ty = quote!(core::mem::MaybeUninit<#ty>);

        items.push(quote! {
            #[cfg(amp_data)]
            #[link_section = ".shared"]
            #[no_mangle]
            static #mut_ #ident: #ty = core::mem::MaybeUninit::uninitialized();
        });

        if static_.mutability.is_some() {
            items.push(quote!(
                #[cfg(amp_shared)]
                extern "C" {
                    static mut #ident: #ty;
                }
            ));
        } else {
            items.push(quote!(
                #[cfg(amp_shared)]
                struct #ident;

                #[cfg(amp_shared)]
                impl core::ops::Deref for #ident {
                    type Target = #ty;

                    fn deref(&self) -> &Self::Target {
                        extern "C" {
                            static #ident: #ty;
                        }

                        unsafe { &#ident }
                    }
                }
            ));
        }
    }

    quote!(#(#items)*).into()
}
