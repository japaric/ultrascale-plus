//! Hello, world!
//!
//! Expected output:
//!
//! ``` text
//! $ tail -f dcc0.log
//! Hello, world!
//! ```

#![no_main]
#![no_std]

extern crate panic_dcc;

use arm_dcc::dprintln;
use zup_rt::entry;

#[entry]
fn main() -> ! {
    dprintln!("Hello, world!");

    loop {}
}
