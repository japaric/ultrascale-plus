#![feature(proc_macro_hygiene)] // required by `dprint*!`
#![no_main]
#![no_std]

use arm_dcc::dprintln;
use panic_dcc as _;
use rtfm::Instant;

#[rtfm::app]
const APP: () = {
    #[idle(spawn = [foo])]
    fn idle(c: idle::Context) -> ! {
        let now = Instant::now();

        c.spawn.foo(now).ok();

        loop {}
    }

    #[task]
    fn foo(_: foo::Context, before: Instant) {
        let now = Instant::now();

        if let Some(dur) = now.checked_duration_since(before) {
            print(dur.as_cycles());
        }
    }
};

#[inline(never)]
fn print(cycles: u32) {
    // 58 cycles
    dprintln!("{}", cycles);
}
