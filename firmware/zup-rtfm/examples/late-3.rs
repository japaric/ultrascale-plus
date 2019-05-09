#![feature(proc_macro_hygiene)] // required by `dprint*!`
#![no_main]
#![no_std]

use arm_dcc::dprintln;
use panic_dcc as _;

#[rtfm::app(cores = 2)]
const APP: () = {
    static mut X: u32 = ();
    static mut Y: u32 = ();

    // core#0 initializes late resource X
    #[init(core = 0, late = [X])]
    fn init(_: init::Context) -> init::LateResources {
        init::LateResources { X: 1 }
    }

    #[idle(core = 0, resources = [Y])]
    fn idle(c: idle::Context) -> ! {
        assert_eq!(*c.resources.Y, 2);

        dprintln!("idle");

        loop {}
    }

    // core#1 initializes the rest of late resources
    #[init(core = 1)]
    fn init(_: init::Context) -> init::LateResources {
        init::LateResources { Y: 2 }
    }

    #[idle(core = 1, resources = [X])]
    fn idle(c: idle::Context) -> ! {
        assert_eq!(*c.resources.X, 1);

        dprintln!("idle");

        loop {}
    }
};
