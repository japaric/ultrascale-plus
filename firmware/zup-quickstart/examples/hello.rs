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

use arm_dcc::dprintln;
use panic_dcc as _;
use zup_rt::entry;

#[entry]
fn main() -> ! {
    dprintln!("Hello, world!");

    loop {}
}
