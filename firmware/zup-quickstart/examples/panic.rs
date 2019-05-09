//! Panicking
//!
//! Expected output:
//!
//! ``` text
//! $ tail -f dcc0.log
//! panicked at 'Oops', zup-quickstart/examples/panic.rs:19:5
//! ```

#![no_main]
#![no_std]

use panic_dcc as _;
use zup_rt::entry;

#[entry]
fn main() -> ! {
    panic!("Oops");
}
