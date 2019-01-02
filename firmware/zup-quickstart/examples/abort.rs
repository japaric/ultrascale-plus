//! Abort
//!
//! Expected output:
//!
//! ``` text
//! $ tail -f dcc0.log
//! Undefined
//! ```

#![feature(core_intrinsics)]
#![no_main]
#![no_std]

extern crate panic_dcc;

use core::intrinsics;

use dcc::dprintln;
use zup_rt::{entry, exception};

#[entry]
fn main() -> ! {
    unsafe { intrinsics::abort() }
}

#[exception]
fn Undefined() -> ! {
    dprintln!("Undefined");

    loop {}
}
