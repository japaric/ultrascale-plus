#![feature(maybe_uninit)]
#![feature(proc_macro_hygiene)] // required by `dprint*!`
#![no_main]
#![no_std]

use core::{
    mem::MaybeUninit,
    sync::atomic::{AtomicBool, Ordering},
};

use microamp::shared;
use panic_dcc as _;
use rtfm::Instant;
use zup_rt::entry;

#[shared]
static RENDEZVOUS: AtomicBool = AtomicBool::new(false);

#[shared]
static mut INSTANT: MaybeUninit<Instant> = MaybeUninit::uninit();

// NOTE Run core#0 first
#[cfg(core = "0")]
#[entry]
fn main() -> ! {
    while !RENDEZVOUS.load(Ordering::Acquire) {}

    let now = Instant::now();
    let earlier: Instant = unsafe { INSTANT.read() };

    if let Some(dur) = now.checked_duration_since(earlier) {
        print(dur.as_cycles());
    }

    loop {}

    #[inline(never)]
    fn print(cycles: u32) {
        use arm_dcc::dprintln;

        // 48-49 cycles
        dprintln!("{}", cycles);
    }
}

#[cfg(core = "1")]
#[entry]
fn main() -> ! {
    rtfm::export::setup_counter();

    unsafe {
        INSTANT.write(Instant::now());
    }

    RENDEZVOUS.store(true, Ordering::Release);

    loop {}
}
