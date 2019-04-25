#![no_main]
#![no_std]

use arm_dcc::dprintln;
use panic_dcc as _;

#[rtfm::app(cores = 2)]
const APP: () = {
    static mut X: u32 = ();
    static mut Y: u32 = ();

    #[init(core = 0)]
    fn init(_: init::Context) {}

    #[idle(core = 0)]
    fn idle(_: idle::Context) -> ! {
        dprintln!("idle");

        loop {}
    }

    // core#1 initializes all late resources
    #[init(core = 1)]
    fn init(_: init::Context) -> init::LateResources {
        init::LateResources { X: 1, Y: 2 }
    }

    #[idle(core = 1, resources = [X, Y])]
    fn idle(c: idle::Context) -> ! {
        assert_eq!(*c.resources.X, 1);
        assert_eq!(*c.resources.Y, 2);

        dprintln!("idle");

        loop {}
    }
};
