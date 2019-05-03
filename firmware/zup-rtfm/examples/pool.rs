//! Using a memory pool in multi-core context
//!
//! Expected output
//!
//! ``` text
//! $ tail -f dcc0.log
//! capacity: 8
//! ping(0xfffd0304)
//! DONE
//! ```
//!
//! ``` text
//! $ tail -f dcc1.log
//! pong(0xfffd0384)
//! ```

#![no_main]
#![no_std]

use arm_dcc::dprintln;
use heapless::{
    pool,
    pool::singleton::{Box, Pool},
};
use panic_dcc as _;

pool!(
    #[microamp::shared]
    A: [u8; 124]
);

#[rtfm::app(cores = 2)]
const APP: () = {
    #[init(core = 0, spawn = [pong])]
    fn init(c: init::Context) {
        // place this buffer in the OCM to avoid having both cores contend for the ATCM_0
        #[ocm]
        static mut X: [u8; 1024] = [0; 1024];

        dprintln!("capacity: {}", A::grow(X));

        // allocate a buffer and clear its contents
        let mut x = A::alloc().expect("alloc").freeze();
        x.iter_mut().for_each(|x| *x = 0);

        c.spawn.pong(x).ok().expect("spawn.pong");
    }

    #[task(core = 0, spawn = [pong])]
    fn ping(c: ping::Context, x: Box<A>) {
        dprintln!("ping({:?})", x.as_ptr());

        assert!(x.iter().all(|x| *x == 0), "buffer is not zeroed");

        drop(x);

        dprintln!("DONE");
    }

    #[task(core = 1, spawn = [ping])]
    fn pong(c: pong::Context, x: Box<A>) {
        dprintln!("pong({:?})", x.as_ptr());

        assert!(x.iter().all(|x| *x == 0), "buffer is not zeroed");

        // allocate a *new* buffer and clear its contents
        let mut y = A::alloc().expect("alloc").freeze();
        y.iter_mut().for_each(|x| *x = 0);

        drop(x);
        c.spawn.ping(y).ok().expect("spawn.ping");
    }
};
