//! Cross core message passing
//!
//! NOTE: make sure you start RPU0 *after* RPU1
//!
//! Expected output
//!
//! ``` text
//! $ tail -f dcc0.log
//! init
//! idle
//! IRQ(ICCIAR { cpuid: 1, ackintid: 0 })
//! ping(1)
//! ~IRQ(ICCIAR { cpuid: 1, ackintid: 0 })
//! IRQ(ICCIAR { cpuid: 1, ackintid: 0 })
//! ping(3)
//! ~IRQ(ICCIAR { cpuid: 1, ackintid: 0 })
//! IRQ(ICCIAR { cpuid: 1, ackintid: 0 })
//! ping(5)
//! ~IRQ(ICCIAR { cpuid: 1, ackintid: 0 })
//! ```
//!
//! Note that the first message is local (cpuid = 1)
//!
//! ```
//! $ tail -f dcc0.log
//! init
//! IRQ(ICCIAR { cpuid: 0, ackintid: 0 })
//! pong(0)
//! ~IRQ(ICCIAR { cpuid: 0, ackintid: 0 })
//! idle
//! IRQ(ICCIAR { cpuid: 0, ackintid: 0 })
//! pong(2)
//! ~IRQ(ICCIAR { cpuid: 0, ackintid: 0 })
//! IRQ(ICCIAR { cpuid: 0, ackintid: 0 })
//! pong(4)
//! ~IRQ(ICCIAR { cpuid: 0, ackintid: 0 })
//! ```

#![feature(proc_macro_hygiene)] // required by `dprint*!`
#![no_main]
#![no_std]

use arm_dcc::dprintln;
use panic_dcc as _;

const LIMIT: u32 = 5;

#[rtfm::app(cores = 2)]
const APP: () = {
    #[init(core = 0, spawn = [pong])]
    fn init(c: init::Context) {
        dprintln!("init");

        c.spawn.pong(0).ok().unwrap();
    }

    #[idle(core = 0)]
    fn idle(_: idle::Context) -> ! {
        dprintln!("idle");

        loop {}
    }

    #[task(core = 0, spawn = [pong])]
    fn ping(c: ping::Context, x: u32) {
        dprintln!("ping({})", x);

        if x < LIMIT {
            c.spawn.pong(x + 1).ok().unwrap();
        }
    }

    #[init(core = 1)]
    fn init(_: init::Context) {
        dprintln!("init");
    }

    #[idle(core = 1)]
    fn idle(_: idle::Context) -> ! {
        dprintln!("idle");

        loop {}
    }

    #[task(core = 1, spawn = [ping])]
    fn pong(c: pong::Context, x: u32) {
        dprintln!("pong({})", x);

        if x < LIMIT {
            c.spawn.ping(x + 1).ok().unwrap();
        }
    }
};
