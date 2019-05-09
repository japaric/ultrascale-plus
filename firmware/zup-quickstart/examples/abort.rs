//! Abort
//!
//! Expected output:
//!
//! ``` text
//! $ tail -f dcc0.log
//! Undefined
//! ```

#![feature(core_intrinsics)]
#![feature(proc_macro_hygiene)] // required by `dprint*!`
#![no_main]
#![no_std]

use core::intrinsics;

use arm_dcc::dprintln;
use panic_dcc as _;
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
