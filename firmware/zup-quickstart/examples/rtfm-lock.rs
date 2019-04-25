//! Port of https://japaric.github.io/cortex-m-rtfm/book/by-example/resources.html#priorities
//!
//! Expected output
//!
//! ```
//! IRQ(ICCIAR { cpuid: 0, ackintid: 2 })
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

#![no_main]
#![no_std]

use arm_dcc::dprintln;
use panic_dcc as _;

#[rtfm::app]
const APP: () = {
    static mut SHARED: u32 = 0;

    // priority = 0
    #[init(spawn = [foo])]
    fn init(c: init::Context) {
        c.spawn.foo().unwrap();
    }

    #[idle]
    fn idle(_: idle::Context) -> ! {
        dprintln!("F");

        loop {}
    }

    // priority = 1
    #[task(resources = [SHARED], spawn = [bar, baz])]
    fn foo(
        foo::Context {
            mut resources,
            spawn,
        }: foo::Context,
    ) {
        dprintln!("A");

        resources.SHARED.lock(|shared| {
            *shared += 1;

            spawn.bar().unwrap();

            dprintln!("B - SHARED = {}", *shared);

            spawn.baz().unwrap();
        });

        dprintln!("E");
    }

    #[task(priority = 2, resources = [SHARED])]
    fn bar(c: bar::Context) {
        *c.resources.SHARED += 1;

        dprintln!("D - SHARED = {}", *c.resources.SHARED);
    }

    #[task(priority = 3)]
    fn baz(_: baz::Context) {
        dprintln!("C");
    }
};
