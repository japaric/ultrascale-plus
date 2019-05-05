#![no_main]
#![no_std]

#[cfg(core = "0")]
use arm_dcc::dprintln;
use panic_dcc as _;
use rtfm::Local;

#[cfg(core = "0")]
const THRESHOLD: usize = 0x3_0000;

#[rtfm::app(cores = 2)]
const APP: () = {
    static mut X: [u8; 1024] = [0; 1024];
    static mut Y: [u8; 1024] = [0; 1024];

    #[init(
        core = 0,
        resources = [X],
        spawn = [foo],
    )]
    fn init(c: init::Context) {
        static mut Z: [u8; 1024] = [0; 1024];

        let x: Local<[u8; 1024]> = c.resources.X;
        let z: Local<[u8; 1024]> = Z;

        x.borrow(|x| {
            assert!((x.as_ptr() as usize) < THRESHOLD);
        });

        z.borrow(|z| {
            assert!((z.as_ptr() as usize) < THRESHOLD);
        });

        dprintln!("init: OK");

        c.spawn.foo(z).ok().unwrap();
    }

    #[idle(core = 0, resources = [Y])]
    fn idle(c: idle::Context) -> ! {
        let y: Local<[u8; 1024]> = c.resources.Y;

        y.borrow(|y| {
            assert!((y.as_ptr() as usize) < THRESHOLD);
        });

        dprintln!("idle: OK");

        loop {}
    }

    #[task(
        core = 0,
        // spawn = [bar], // error: `Local<_>` can't cross the core boundary
    )]
    fn foo(_: foo::Context, x: Local<[u8; 1024]>) {
        x.borrow(|x| {
            assert!((x.as_ptr() as usize) < THRESHOLD);

            assert!(x.iter().all(|x| *x == 0));
        });

        dprintln!("foo: OK");
    }

    #[task(core = 1)]
    fn bar(_: bar::Context, _: Local<[u8; 1024]>) {}
};
