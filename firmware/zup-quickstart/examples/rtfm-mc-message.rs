//! `rtfm-message` example but duplicated on each core
//!
//! Expected output (core 0):
//!
//! ```
//! $ tail -f dcc0.log
//! IRQ(ICCIAR { cpuid: 0, ackintid: 0 })
//! foo
//! bar(0)
//! baz(1, 2)
//! foo
//! bar(1)
//! baz(2, 3)
//! ~IRQ(ICCIAR { cpuid: 0, ackintid: 0 })
//! IRQ(ICCIAR { cpuid: 0, ackintid: 0 })
//! ~IRQ(ICCIAR { cpuid: 0, ackintid: 0 })
//! ```
//!
//! ```
//! $ tail -f dcc0.log
//! IRQ(ICCIAR { cpuid: 1, ackintid: 0 })
//! foo
//! bar(0)
//! baz(1, 2)
//! foo
//! bar(1)
//! baz(2, 3)
//! ~IRQ(ICCIAR { cpuid: 1, ackintid: 0 })
//! IRQ(ICCIAR { cpuid: 1, ackintid: 0 })
//! ~IRQ(ICCIAR { cpuid: 1, ackintid: 0 })
//! ```

#![feature(maybe_uninit)]
#![no_main]
#![no_std]

extern crate panic_dcc;

use arm_dcc::dprintln;
use rtfm::app;

#[app(cores = 2)]
const APP: () = {
    #[cfg(core = "0")]
    #[init(spawn = [foo0])]
    fn init() {
        spawn.foo0(/* no message */).unwrap();
    }

    #[cfg(core = "0")]
    #[task(spawn = [bar0])]
    fn foo0() {
        static mut COUNT: u32 = 0;

        dprintln!("foo");

        spawn.bar0(*COUNT).unwrap();
        *COUNT += 1;
    }

    #[cfg(core = "0")]
    #[task(spawn = [baz0])]
    fn bar0(x: u32) {
        dprintln!("bar({})", x);

        spawn.baz0(x + 1, x + 2).unwrap();
    }

    #[cfg(core = "0")]
    #[task(spawn = [foo0])]
    fn baz0(x: u32, y: u32) {
        dprintln!("baz({}, {})", x, y);

        if x + y <= 4 {
            spawn.foo0().unwrap();
        }
    }

    #[cfg(core = "1")]
    #[init(spawn = [foo1])]
    fn init() {
        spawn.foo1(/* no message */).unwrap();
    }

    #[cfg(core = "1")]
    #[task(spawn = [bar1])]
    fn foo1() {
        static mut COUNT: u32 = 0;

        dprintln!("foo");

        spawn.bar1(*COUNT).unwrap();
        *COUNT += 1;
    }

    #[cfg(core = "1")]
    #[task(spawn = [baz1])]
    fn bar1(x: u32) {
        dprintln!("bar({})", x);

        spawn.baz1(x + 1, x + 2).unwrap();
    }

    #[cfg(core = "1")]
    #[task(spawn = [foo1])]
    fn baz1(x: u32, y: u32) {
        dprintln!("baz({}, {})", x, y);

        if x + y <= 4 {
            spawn.foo1().unwrap();
        }
    }
};
