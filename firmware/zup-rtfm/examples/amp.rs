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
//! $ tail -f dcc1.log
//! 1
//! 3
//! 5
//! 7
//! 9
//! DONE
//! ```

#![no_main]
#![no_std]

use core::sync::atomic::{AtomicUsize, Ordering};

use arm_dcc::dprintln;
use microamp::shared;
// use panic_halt as _;
use panic_dcc as _;
use zup_rt::entry;

// possible values of SEMAPHORE
const CORE0: usize = 0;
const CORE1: usize = 1;
const LOCKED: usize = 2;

// used as a mutex
#[shared] // <- means: shared between all the cores
static SEMAPHORE: AtomicUsize = AtomicUsize::new(CORE0);

#[shared]
static mut SHARED: usize = 0;

#[entry]
fn main() -> ! {
    #[cfg(core = "0")]
    const OUR_TURN: usize = CORE0;
    #[cfg(core = "0")]
    const OTHER_CORE: usize = CORE1;

    #[cfg(not(core = "0"))]
    const OUR_TURN: usize = CORE1;
    #[cfg(not(core = "0"))]
    const OTHER_CORE: usize = CORE0;

    dprintln!("START");

    let mut done = false;
    while !done {
        // try to acquire the lock
        while SEMAPHORE.compare_and_swap(OUR_TURN, LOCKED, Ordering::AcqRel) != OUR_TURN {
            // spin wait
        }

        // we acquired the lock; now we have exclusive access to `SHARED`
        unsafe {
            if SHARED >= 10 {
                done = true;
            } else {
                dprintln!("{}", SHARED);

                SHARED += 1;
            }
        }

        // release the lock / unblock the other core
        SEMAPHORE.store(OTHER_CORE, Ordering::Release);
    }

    dprintln!("DONE");

    loop {}
}
