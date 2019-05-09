#![feature(proc_macro_hygiene)] // required by `dprint*!`
#![no_main]
#![no_std]

use arm_dcc::dprintln;
use panic_dcc as _;
use rtfm::Instant;

#[rtfm::app]
const APP: () = {
    #[init(spawn = [foo])]
    fn init(c: init::Context) {
        c.spawn.foo().unwrap();
    }

    #[task(priority = 2, spawn = [bar])]
    fn foo(c: foo::Context) {
        let start = Instant::now();
        c.spawn.bar(0).ok();
        let end = Instant::now();

        if let Some(dur) = end.checked_duration_since(start) {
            print(dur.as_cycles());
        }
    }

    #[task]
    fn bar(_: bar::Context, x: u32) {
        dprintln!("bar({})", x);
    }
};

#[inline(never)]
fn print(cycles: u32) {
    // 33 cycles
    dprintln!("{}", cycles);
}
