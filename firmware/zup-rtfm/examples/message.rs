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
//! ```

#![no_main]
#![no_std]

extern crate panic_dcc;

use arm_dcc::dprintln;
use rtfm::app;

#[app(cores = 2)]
const APP: () = {
    #[init(core = 0, spawn = [foo0])]
    fn init(c: init::Context) {
        c.spawn.foo0(/* no message */).unwrap();
    }

    #[task(core = 0, spawn = [bar0])]
    fn foo0(c: foo0::Context) {
        static mut COUNT: u32 = 0;

        dprintln!("foo");

        c.spawn.bar0(*COUNT).unwrap();
        *COUNT += 1;
    }

    #[task(core = 0, spawn = [baz0])]
    fn bar0(c: bar0::Context, x: u32) {
        dprintln!("bar({})", x);

        c.spawn.baz0(x + 1, x + 2).unwrap();
    }

    #[task(core = 0, spawn = [foo0])]
    fn baz0(c: baz0::Context, x: u32, y: u32) {
        dprintln!("baz({}, {})", x, y);

        if x + y <= 4 {
            c.spawn.foo0().unwrap();
        }
    }

    #[init(core = 1, spawn = [foo1])]
    fn init(c: init::Context) {
        c.spawn.foo1(/* no message */).unwrap();
    }

    #[task(core = 1, spawn = [bar1])]
    fn foo1(c: foo1::Context) {
        static mut COUNT: u32 = 0;

        dprintln!("foo");

        c.spawn.bar1(*COUNT).unwrap();
        *COUNT += 1;
    }

    #[task(core = 1, spawn = [baz1])]
    fn bar1(c: bar1::Context, x: u32) {
        dprintln!("bar({})", x);

        c.spawn.baz1(x + 1, x + 2).unwrap();
    }

    #[task(core = 1, spawn = [foo1])]
    fn baz1(c: baz1::Context, x: u32, y: u32) {
        dprintln!("baz({}, {})", x, y);

        if x + y <= 4 {
            c.spawn.foo1().unwrap();
        }
    }
};
