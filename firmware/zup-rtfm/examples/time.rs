#![no_main]
#![no_std]

use arm_dcc::dprintln;
use panic_dcc as _;
use rtfm::{Duration, Instant};

#[rtfm::app(cores = 2)]
const APP: () = {
    #[init(core = 0, spawn = [ping])]
    fn init(c: init::Context) {
        c.spawn.ping().unwrap();
    }

    #[task(core = 0, spawn = [pong])]
    fn ping(c: ping::Context) {
        let now = Instant::now();

        c.spawn.pong(now).ok();
    }

    #[task(core = 1)]
    fn pong(_: pong::Context, earlier: Instant) {
        let now = Instant::now();

        if let Some(dur) = now.checked_duration_since(earlier) {
            print(dur);
        }
    }
};

#[inline(never)]
fn print(dur: Duration) {
    // 162 cycles
    dprintln!("{}", dur.as_cycles());
}
