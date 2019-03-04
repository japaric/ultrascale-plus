//! Single source Asymmetric Multi-Processing (AMP)
//!
//! NOTE: core 0 must start executing the program before core 1
//!
//! Expected output
//!
//! ``` text
//! $ tail -f dcc0.log
//! 0
//! 2
//! 4
//! 6
//! 8
//! DONE
//! ```
//!
//! ``` text
//! $ tail -f dcc0.log
//! 1
//! 3
//! 5
//! 7
//! 9
//! DONE
//! ```

#![feature(maybe_uninit)]
#![no_main]
#![no_std]

extern crate panic_dcc;

use core::sync::atomic::{AtomicUsize, Ordering};

use arm_dcc::dprintln;
use zup_rt::entry;

// possible values of SEMAPHORE
const CORE0: usize = 0;
const CORE1: usize = 1;
const LOCKED: usize = 2;

#[microamp::app]
const APP: () = {
    // used as a mutex
    #[shared] // <- means: shared between all the cores
    static SEMAPHORE: AtomicUsize = ();

    #[shared]
    static mut SHARED: usize = ();

    #[entry]
    fn main() -> ! {
        // `cfg(core)` can only be used within the `#[app]` block
        #[cfg(core = "0")]
        const OUR_TURN: usize = CORE0;
        #[cfg(core = "0")]
        const OTHER_CORE: usize = CORE1;

        #[cfg(core = "1")]
        const OUR_TURN: usize = CORE1;
        #[cfg(core = "1")]
        const OTHER_CORE: usize = CORE0;

        // let one core initialize the `#[shared]` statics
        #[cfg(core = "0")]
        unsafe {
            SHARED.set(0);
            SEMAPHORE.get_ref().store(OUR_TURN, Ordering::Release);
        }

        let semaphore = unsafe { SEMAPHORE.get_ref() };

        let mut done = false;
        while !done {
            // try to acquire the lock
            while semaphore.compare_and_swap(OUR_TURN, LOCKED, Ordering::AcqRel) != OUR_TURN {
                // spin wait
            }

            // we acquired the lock; now we have exclusive access to `SHARED`
            unsafe {
                let shared = SHARED.get_mut();

                if *shared >= 10 {
                    done = true;
                } else {
                    dprintln!("{}", *shared);

                    *shared += 1;
                }
            }

            // release the lock / unblock the other core
            semaphore.store(OTHER_CORE, Ordering::Release);
        }

        dprintln!("DONE");

        loop {}
    }
};
