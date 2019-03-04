//! A SGI that preempts the IRQ handler
//!
//! Expected output:
//!
//! ``` text
//! $ tail -f dcc0.log
//! IRQ(ICCIAR { cpuid: 0, ackintid: 0 })
//! SG0: before SGI
//! IRQ(ICCIAR { cpuid: 0, ackintid: 1 })
//! SG1
//! ~IRQ(ICCIAR { cpuid: 0, ackintid: 1 })
//! SG0: after SGI
//! ~IRQ(ICCIAR { cpuid: 0, ackintid: 0 })
//! ```

#![no_main]
#![no_std]

extern crate panic_dcc;

use cortex_r::gic::{Target, ICC, ICD};
use arm_dcc::dprintln;
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

        // set the priority of SG1 to the third lowest priority
        icd.ICDIPR[1].write(232);

        // enable interrupt signaling
        icc.ICCICR
            .write((1 << 1) /* EnableNS */ | (1 << 0) /* EnableS */);

        // enable interrupt routing
        icd.enable();

        // unmask IRQ
        cortex_r::enable_irq();

        // trigger SG0
        ICD::icdsgir(Target::Loopback, 0);

        // IRQ serviced here
    }

    loop {}
}

#[interrupt]
fn SG0() {
    dprintln!("SG0: before SGI");

    // this SGI will preempt the current IRQ handler
    ICD::icdsgir(Target::Loopback, 1);

    dprintln!("SG0: after SGI");
}

#[interrupt]
fn SG1() {
    dprintln!("SG1");
}
