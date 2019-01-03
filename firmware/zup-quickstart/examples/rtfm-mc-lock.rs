//! `rtfm-lock` example but duplicated on each core
//!
//! Expected output:
//!
//! ```
//! $ tail -f dcc0.log
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
//!
//! ```
//! $ tail -f dcc0.log
//! IRQ(ICCIAR { cpuid: 1, ackintid: 0 })
//! A
//! B - SHARED = 1
//! IRQ(ICCIAR { cpuid: 1, ackintid: 2 })
//! C
//! ~IRQ(ICCIAR { cpuid: 1, ackintid: 2 })
//! IRQ(ICCIAR { cpuid: 1, ackintid: 1 })
//! D - SHARED = 2
//! ~IRQ(ICCIAR { cpuid: 1, ackintid: 1 })
//! E
//! ~IRQ(ICCIAR { cpuid: 1, ackintid: 0 })
//! ```

#![feature(maybe_uninit)]
#![no_main]
#![no_std]

extern crate panic_dcc;

use dcc::dprintln;
use rtfm::app;

#[app(cores = 2)]
const APP: () = {
    #[cfg(core = "0")]
    static mut SHARED0: u32 = 0;

    // priority = 1
    #[cfg(core = "0")]
    #[init(spawn = [foo0])]
    fn init() {
        spawn.foo0().unwrap();
    }

    // priority = 2
    #[cfg(core = "0")]
    #[task(resources = [SHARED0], spawn = [bar0, baz0])]
    fn foo0() {
        dprintln!("A");

        resources.SHARED0.lock(|shared| {
            *shared += 1;

            spawn.bar0().unwrap();

            dprintln!("B - SHARED = {}", *shared);

            spawn.baz0().unwrap();
        });

        dprintln!("E");
    }

    #[cfg(core = "0")]
    #[task(priority = 3, resources = [SHARED0])]
    fn bar0() {
        *resources.SHARED0 += 1;

        dprintln!("D - SHARED = {}", *resources.SHARED0);
    }

    #[cfg(core = "0")]
    #[task(priority = 4)]
    fn baz0() {
        dprintln!("C");
    }

    #[cfg(core = "1")]
    static mut SHARED1: u32 = 0;

    // priority = 1
    #[cfg(core = "1")]
    #[init(spawn = [foo1])]
    fn init() {
        spawn.foo1().unwrap();
    }

    // priority = 2
    #[cfg(core = "1")]
    #[task(resources = [SHARED1], spawn = [bar1, baz1])]
    fn foo1() {
        dprintln!("A");

        resources.SHARED1.lock(|shared| {
            *shared += 1;

            spawn.bar1().unwrap();

            dprintln!("B - SHARED = {}", *shared);

            spawn.baz1().unwrap();
        });

        dprintln!("E");
    }

    #[cfg(core = "1")]
    #[task(priority = 3, resources = [SHARED1])]
    fn bar1() {
        *resources.SHARED1 += 1;

        dprintln!("D - SHARED = {}", *resources.SHARED1);
    }

    #[cfg(core = "1")]
    #[task(priority = 4)]
    fn baz1() {
        dprintln!("C");
    }
};
