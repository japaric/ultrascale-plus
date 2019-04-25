//! `rtfm-lock` example but duplicated on each core
//!
//! Expected output:
//!
//! ```
//! $ tail -f dcc0.log
// IRQ(ICCIAR { cpuid: 0, ackintid: 2 })
//! A
//! B - SHARED = 1
//! IRQ(ICCIAR { cpuid: 0, ackintid: 1 })
//! C
//! ~IRQ(ICCIAR { cpuid: 0, ackintid: 1 })
//! IRQ(ICCIAR { cpuid: 0, ackintid: 0 })
//! D - SHARED = 2
//! ~IRQ(ICCIAR { cpuid: 0, ackintid: 0 })
//! E
//! ~IRQ(ICCIAR { cpuid: 0, ackintid: 2 })
//! F
//! ```
//!
//! ```
//! $ tail -f dcc0.log
//! IRQ(ICCIAR { cpuid: 1, ackintid: 2 })
//! A
//! B - SHARED = 1
//! IRQ(ICCIAR { cpuid: 1, ackintid: 1 })
//! C
//! ~IRQ(ICCIAR { cpuid: 1, ackintid: 1 })
//! IRQ(ICCIAR { cpuid: 1, ackintid: 0 })
//! D - SHARED = 2
//! ~IRQ(ICCIAR { cpuid: 1, ackintid: 0 })
//! E
//! ~IRQ(ICCIAR { cpuid: 1, ackintid: 2 })
//! F
//! ```

#![no_main]
#![no_std]

use arm_dcc::dprintln;
use panic_dcc as _;

#[rtfm::app(cores = 2)]
const APP: () = {
    static mut SHARED0: u32 = 0;

    // priority = 0
    #[init(core = 0, spawn = [foo0])]
    fn init(c: init::Context) {
        c.spawn.foo0().unwrap();
    }

    #[idle(core = 0)]
    fn idle(_: idle::Context) -> ! {
        dprintln!("F");

        loop {}
    }

    // priority = 1
    #[task(core = 0, resources = [SHARED0], spawn = [bar0, baz0])]
    fn foo0(
        foo0::Context {
            spawn,
            mut resources,
        }: foo0::Context,
    ) {
        dprintln!("A");

        resources.SHARED0.lock(|shared| {
            *shared += 1;

            spawn.bar0().unwrap();

            dprintln!("B - SHARED = {}", *shared);

            spawn.baz0().unwrap();
        });

        dprintln!("E");
    }

    #[task(core = 0, priority = 2, resources = [SHARED0])]
    fn bar0(c: bar0::Context) {
        *c.resources.SHARED0 += 1;

        dprintln!("D - SHARED = {}", c.resources.SHARED0);
    }

    #[task(core = 0, priority = 3)]
    fn baz0(_: baz0::Context) {
        dprintln!("C");
    }

    static mut SHARED1: u32 = 0;

    // priority = 0
    #[init(core = 1, spawn = [foo1])]
    fn init(c: init::Context) {
        c.spawn.foo1().unwrap();
    }

    #[idle(core = 1)]
    fn idle(_: idle::Context) -> ! {
        dprintln!("F");

        loop {}
    }

    // priority = 1
    #[task(core = 1, resources = [SHARED1], spawn = [bar1, baz1])]
    fn foo1(
        foo1::Context {
            spawn,
            mut resources,
        }: foo1::Context,
    ) {
        dprintln!("A");

        resources.SHARED1.lock(|shared| {
            *shared += 1;

            spawn.bar1().unwrap();

            dprintln!("B - SHARED = {}", *shared);

            spawn.baz1().unwrap();
        });

        dprintln!("E");
    }

    #[task(core = 1, priority = 2, resources = [SHARED1])]
    fn bar1(c: bar1::Context) {
        *c.resources.SHARED1 += 1;

        dprintln!("D - SHARED = {}", c.resources.SHARED1);
    }

    #[task(core = 1, priority = 3)]
    fn baz1(_: baz1::Context) {
        dprintln!("C");
    }
};
