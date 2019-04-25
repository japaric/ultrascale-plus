#![no_main]
#![no_std]

use arm_dcc::dprintln;
use cortex_r::gic::{Target, ICD};
use panic_dcc as _;

#[rtfm::app]
const APP: () = {
    #[init]
    fn init(_: init::Context) {
        ICD::icdsgir(Target::Loopback, 0);
    }

    #[interrupt]
    fn SG0(_: SG0::Context) {
        dprintln!("SG0");
    }
};
