//! Cross core message passing
//!
//! NOTE: make sure you start RPU0 *after* RPU1
//!
//! Expected output
//!
//! ``` text
//! $ tail -f dcc0.log
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
//! IRQ(ICCIAR { cpuid: 1, ackintid: 0 })
//! pong(0)
//! ~IRQ(ICCIAR { cpuid: 1, ackintid: 0 })
//! IRQ(ICCIAR { cpuid: 0, ackintid: 0 })
//! pong(2)
//! ~IRQ(ICCIAR { cpuid: 0, ackintid: 0 })
//! IRQ(ICCIAR { cpuid: 0, ackintid: 0 })
//! pong(4)
//! ~IRQ(ICCIAR { cpuid: 0, ackintid: 0 })
//! ```

#![feature(maybe_uninit)]
#![no_main]
#![no_std]

extern crate panic_dcc;

use dcc::dprintln;
use rtfm::app;

const LIMIT: u32 = 5;

#[app(cores = 2)]
const APP: () = {
    #[cfg(core = "0")]
    #[init]
    fn init() {}

    #[cfg(core = "1")]
    #[init(spawn = [pong])]
    fn init() {
        spawn.pong(0).unwrap();
    }

    #[cfg(core = "0")]
    #[task(spawn = [pong])]
    fn ping(x: u32) {
        dprintln!("ping({})", x);

        if x < LIMIT {
            spawn.pong(x + 1).unwrap();
        }
    }

    #[cfg(core = "1")]
    #[task(spawn = [ping])]
    fn pong(x: u32) {
        dprintln!("pong({})", x);

        if x < LIMIT {
            spawn.ping(x + 1).unwrap();
        }
    }
};
