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

#![feature(proc_macro_hygiene)] // required by `dprint*!`
#![no_main]
#![no_std]

use core::sync::atomic::{AtomicU8, Ordering};

use arm_dcc::dprintln;
use microamp::shared;
// use panic_halt as _;
use panic_dcc as _;
use zup_rt::entry;

// non-atomic variable
#[shared] // <- means: visible to all the cores
static mut SHARED: u64 = 0;

// used to synchronize access to `SHARED`
#[shared]
static SEMAPHORE: AtomicU8 = AtomicU8::new(CORE0);

// possible values of SEMAPHORE
const CORE0: u8 = 0;
const CORE1: u8 = 1;
const LOCKED: u8 = 2;

#[entry]
fn main() -> ! {
    let (our_turn, next_core) = if cfg!(core = "0") {
        (CORE0, CORE1)
    } else {
        (CORE1, CORE0)
    };

    dprintln!("START");

    let mut done = false;
    while !done {
        // try to acquire the lock
        while SEMAPHORE
            .compare_exchange(our_turn, LOCKED, Ordering::AcqRel, Ordering::Relaxed)
            .is_err()
        {
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

        // release the lock & unblock the other core
        SEMAPHORE.store(next_core, Ordering::Release);
    }

    dprintln!("DONE");

    loop {}
}
