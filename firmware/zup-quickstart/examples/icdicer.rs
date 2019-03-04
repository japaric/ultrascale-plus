//! Find out whether SGIs can be disabled or not
//!
//! Expected output:
//!
//! ``` text
//! $ tail -f dcc0.log
//! 0xffff
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
    icd.ICDICER[0].write(!0);
    dprintln!("{:#x}", icd.ICDICER[0].read());

    loop {}
}
