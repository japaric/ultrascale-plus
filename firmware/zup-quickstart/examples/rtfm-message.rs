//! Port of https://japaric.github.io/cortex-m-rtfm/book/by-example/tasks.html#message-passing
//!
//! Expected output:
//!
//! ```
//! init
//! IRQ(ICCIAR { cpuid: 0, ackintid: 0 })
//! foo
//! bar(0)
//! baz(1, 2)
//! foo
//! bar(1)
//! baz(2, 3)
//! ~IRQ(ICCIAR { cpuid: 0, ackintid: 0 })
//! idle
//! ```

#![no_main]
#![no_std]

use arm_dcc::dprintln;
use panic_dcc as _;

#[rtfm::app]
const APP: () = {
    #[init(spawn = [foo])]
    fn init(c: init::Context) {
        c.spawn.foo(/* no message */).unwrap();

        dprintln!("init");
    }

    #[idle]
    fn idle(_: idle::Context) -> ! {
        dprintln!("idle");

        loop {}
    }

    #[task(spawn = [bar])]
    fn foo(c: foo::Context) {
        static mut COUNT: u32 = 0;

        dprintln!("foo");

        c.spawn.bar(*COUNT).unwrap();
        *COUNT += 1;
    }

    #[task(spawn = [baz])]
    fn bar(c: bar::Context, x: u32) {
        dprintln!("bar({})", x);

        c.spawn.baz(x + 1, x + 2).unwrap();
    }

    #[task(spawn = [foo])]
    fn baz(c: baz::Context, x: u32, y: u32) {
        dprintln!("baz({}, {})", x, y);

        if x + y <= 4 {
            c.spawn.foo().unwrap();
        }
    }
};
