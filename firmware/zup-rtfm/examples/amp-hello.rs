//! AMP-style hello world
//!
//! Expected output:
//!
//! ```
//! $ # output of core #0
//! $ tail -f dcc0.log
//! Hello from core 0
//! X has address 0xffe217ac
//! ```
//!
//! ```
//! $ # output of core #1
//! $ tail -f dcc0.log
//! Hello from core 1
//! X has address 0xffe217ac
//! ```

#![no_main]
#![no_std]

use arm_dcc::dprintln;
use panic_dcc as _; // panic handler
use zup_rt::entry;

// program entry point
#[entry]
fn main() -> ! {
    static mut X: u32 = 0;

    // `#[entry]` transforms `X` into a `&'static mut` reference
    let x: &'static mut u32 = X;

    let who_am_i = if cfg!(core = "0") { 0 } else { 1 };
    dprintln!("Hello from core {}", who_am_i);

    dprintln!("X has address {:?}", x as *mut u32);

    loop {}
}
