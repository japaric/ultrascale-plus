#![feature(proc_macro_hygiene)] // required by `dprint*!`
#![no_main]
#![no_std]

use core::sync::atomic::AtomicBool;

#[cfg(core = "0")]
use arm_dcc::dprintln;
use panic_dcc as _;
use rtfm::{LocalMut, LocalRef};

#[cfg(core = "0")]
const THRESHOLD: usize = 0x3_0000;

#[rtfm::app(cores = 2)]
const APP: () = {
    static A: AtomicBool = AtomicBool::new(false);
    static B: AtomicBool = AtomicBool::new(false);
    static mut X: [u8; 1024] = [0; 1024];
    static mut Y: [u8; 1024] = [0; 1024];

    #[init(
        core = 0,
        resources = [A, X],
        spawn = [foo],
    )]
    fn init(c: init::Context) {
        static mut Z: [u8; 1024] = [0; 1024];

        let a: LocalRef<AtomicBool> = c.resources.A;
        let x: LocalMut<[u8; 1024]> = c.resources.X;
        let z: LocalMut<[u8; 1024]> = Z;

        a.borrow(|a| {
            assert!((a as *const AtomicBool as usize) < THRESHOLD);
        });

        x.borrow(|x| {
            assert!((x.as_ptr() as usize) < THRESHOLD);
        });

        z.borrow(|z| {
            assert!((z.as_ptr() as usize) < THRESHOLD);
        });

        dprintln!("init: OK");

        c.spawn.foo(z).ok().unwrap();
    }

    #[idle(core = 0, resources = [B, Y])]
    fn idle(c: idle::Context) -> ! {
        let b: LocalRef<AtomicBool> = c.resources.B;
        let y: LocalMut<[u8; 1024]> = c.resources.Y;

        b.borrow(|b| {
            assert!((b as *const AtomicBool as usize) < THRESHOLD);
        });

        y.borrow(|y| {
            assert!((y.as_ptr() as usize) < THRESHOLD);
        });

        dprintln!("idle: OK");

        loop {}
    }

    #[task(
        core = 0,
        // spawn = [baz], //~ error: `LocalMut<_>` can't cross the core boundary
    )]
    fn foo(_: foo::Context, x: LocalMut<[u8; 1024]>) {
        x.borrow(|x| {
            assert!((x.as_ptr() as usize) < THRESHOLD);

            assert!(x.iter().all(|x| *x == 0));
        });

        dprintln!("foo: OK");
    }

    #[task(
        core = 0,
        // spawn = [quux], //~ error: `LocalRef<_>` can't cross the core boundary
    )]
    fn bar(_: bar::Context, x: LocalRef<AtomicBool>) {
        x.borrow(|x| {
            assert!((x as *const AtomicBool as usize) < THRESHOLD);
        });
    }

    #[task(core = 1)]
    fn baz(_: baz::Context, _: LocalRef<AtomicBool>) {}

    #[task(core = 1)]
    fn quux(_: quux::Context, _: LocalMut<[u8; 1024]>) {}
};
