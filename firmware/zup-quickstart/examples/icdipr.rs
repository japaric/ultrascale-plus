//! Find out the number of supported priority bits
//!
//! Expected output:
//!
//! ``` text
//! $ tail -f dcc0.log
//! 0b11111000
//! ```

#![no_main]
#![no_std]

extern crate panic_dcc;

use cortex_r::gic::ICD;
use arm_dcc::dprintln;
use zup_rt::entry;

#[entry]
unsafe fn main() -> ! {
    let mut icd = ICD::steal();
    // disable routing during configuration phase
    icd.disable();
    icd.ICDIPR[0].write(0xff);
    dprintln!("{:#b}", icd.ICDIPR[0].read());

    loop {}
}
