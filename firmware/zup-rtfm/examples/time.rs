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
        let x = Instant::now();
        c.spawn.pong(x).ok();
        let y = Instant::now();

        if let Some(dur) = y.checked_duration_since(x) {
            print(dur);
        }
    }

    #[task(core = 1)]
    fn pong(_: pong::Context, x: Instant) {
        let z = Instant::now();

        if let Some(dur) = z.checked_duration_since(x) {
            print(dur);
        }
    }
};

#[inline(never)]
fn print(dur: Duration) {
    // x -> y:  84 cycles
    // x -> z: 156 cycles
    dprintln!("{}", dur.as_cycles());
}
