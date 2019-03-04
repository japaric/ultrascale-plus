//! Port of https://japaric.github.io/cortex-m-rtfm/book/by-example/resources.html#priorities
//!
//! Expected output
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

#![feature(maybe_uninit)]
#![no_main]
#![no_std]

extern crate panic_dcc;

use arm_dcc::dprintln;

#[rtfm::app]
const APP: () = {
    static mut SHARED: u32 = 0;

    #[init(spawn = [foo])]
    fn init() {
        spawn.foo().unwrap();
    }

    // priority = 2
    #[task(resources = [SHARED], spawn = [bar, baz])]
    fn foo() {
        dprintln!("A");

        resources.SHARED.lock(|shared| {
            *shared += 1;

            spawn.bar().unwrap();

            dprintln!("B - SHARED = {}", *shared);

            spawn.baz().unwrap();
        });

        dprintln!("E");
    }

    #[task(priority = 3, resources = [SHARED])]
    fn bar() {
        *resources.SHARED += 1;

        dprintln!("D - SHARED = {}", *resources.SHARED);
    }

    #[task(priority = 4)]
    fn baz() {
        dprintln!("C");
    }
};
