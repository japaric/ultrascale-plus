//! Port of https://japaric.github.io/cortex-m-rtfm/book/by-example/tasks.html#message-passing
//!
//! Expected output:
//!
//! ```
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

#![feature(maybe_uninit)]
#![no_main]
#![no_std]

extern crate panic_dcc;

use arm_dcc::dprintln;

#[rtfm::app]
const APP: () = {
    #[init(spawn = [foo])]
    fn init() {
        spawn.foo(/* no message */).unwrap();
    }

    #[task(spawn = [bar])]
    fn foo() {
        static mut COUNT: u32 = 0;

        dprintln!("foo");

        spawn.bar(*COUNT).unwrap();
        *COUNT += 1;
    }

    #[task(spawn = [baz])]
    fn bar(x: u32) {
        dprintln!("bar({})", x);

        spawn.baz(x + 1, x + 2).unwrap();
    }

    #[task(spawn = [foo])]
    fn baz(x: u32, y: u32) {
        dprintln!("baz({}, {})", x, y);

        if x + y <= 4 {
            spawn.foo().unwrap();
        }
    }
};
