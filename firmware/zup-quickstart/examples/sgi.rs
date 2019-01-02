//! Generate and dispatch a SGI (Software Generated Interrupt)
//!
//! Expected output:
//!
//! ``` text
//! $ tail -f dcc0.log
//! before SGI. ICDISPR = 0
//! after SGI. ICDISPR = 1
//! IRQ(ICCIAR { cpuid: 0, ackintid: 0 })
//! SG0
//! ~IRQ(ICCIAR { cpuid: 0, ackintid: 0 })
//! after ACK. ICDISPR = 0
//! ```

#![no_main]
#![no_std]

extern crate panic_dcc;

#[cfg(not(debug_assertions))]
use core::sync::atomic::{self, Ordering};

use cortex_r::gic::{Target, ICC, ICD};
use dcc::dprintln;
use zup_rt::{entry, interrupt};

#[entry]
fn main() -> ! {
    unsafe {
        let mut icd = ICD::take().unwrap();
        let mut icc = ICC::take().unwrap();

        // disable interrupt routing and signaling during configuration
        icd.disable();
        icc.disable();

        // set priority mask to the lowest priority
        icc.ICCPMR.write(248);

        // set the priority of SG0 to the second lowest priority
        icd.ICDIPR[0].write(240);

        // enable interrupt signaling
        icc.ICCICR
            .write((1 << 1) /* EnableNS */ | (1 << 0) /* EnableS */);

        // enable interrupt routing
        icd.enable();

        dprintln!("before SGI. ICDISPR = {}", icd.ICDISPR[0].read());

        // trigger a SGI
        ICD::icdsgir(Target::Loopback, 0);

        dprintln!("after SGI. ICDISPR = {}", icd.ICDISPR[0].read());

        // unmask IRQ
        cortex_r::enable_irq();

        // IRQ serviced here

        dprintln!("after ACK. ICDISPR = {}", icd.ICDISPR[0].read());
    }

    loop {
        #[cfg(not(debug_assertions))]
        atomic::compiler_fence(Ordering::SeqCst);
    }
}

#[interrupt]
fn SG0() {
    dprintln!("SG0");
}
