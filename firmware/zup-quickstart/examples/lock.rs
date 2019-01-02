//! Testing resource / BASEPRI like functionality
//!
//! This is a port of https://japaric.github.io/cortex-m-rtfm/book/by-example/resources.html
//!
//! Expected output:
//!
//! ```
//! IRQ(ICCIAR { cpuid: 0, ackintid: 0 })
//! A
//! B - SHARED = 1
//! IRQ(ICCIAR { cpuid: 0, ackintid: 2 })
//! C
//! ~IRQ(ICCIAR { cpuid: 0, ackintid: 2 })
//! IRQ(ICCIAR { cpuid: 0, ackintid: 1 })
//! D - SHARED = 2
//! ~IRQ(ICCIAR { cpuid: 0, ackintid: 1 })
//! E
//! ~IRQ(ICCIAR { cpuid: 0, ackintid: 0 })
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
        ICC::set_iccpmr(248);

        // set the priority of SG0 to the second lowest priority
        icd.ICDIPR[0].write(240);

        // set the priority of SG1 to the third lowest priority
        icd.ICDIPR[1].write(232);

        // set the priority of SG2 to the fourth lowest priority
        icd.ICDIPR[2].write(224);

        // enable interrupt signaling
        icc.ICCICR
            .write((1 << 1) /* EnableNS */ | (1 << 0) /* EnableS */);

        // enable interrupt routing
        icd.enable();

        // unmask IRQ
        cortex_r::enable_irq();

        // trigger a SGI
        ICD::icdsgir(Target::Loopback, 0);

        // IRQ serviced here
    }

    loop {
        #[cfg(not(debug_assertions))]
        atomic::compiler_fence(Ordering::SeqCst);
    }
}

static mut SHARED: u32 = 0;

// priority = 1
#[interrupt]
unsafe fn SG0() {
    dprintln!("A");

    let curr = ICC::get_iccpmr();
    ICC::set_iccpmr(232); // start critical section
    SHARED += 1;

    // SG1 will *not* run due to the critical section
    ICD::icdsgir(Target::Loopback, 1);

    dprintln!("B - SHARED = {}", SHARED);

    // SG2 does not contend for `SHARED` so it's allowed to run now
    ICD::icdsgir(Target::Loopback, 2);

    ICC::set_iccpmr(curr); // end critical section

    // critical section is over: SG1 can now start

    dprintln!("E");
}

// priority = 2
#[interrupt]
unsafe fn SG1() {
    SHARED += 1;

    dprintln!("D - SHARED = {}", SHARED);
}

// priority = 3
#[interrupt]
fn SG2() {
    dprintln!("C");
}
