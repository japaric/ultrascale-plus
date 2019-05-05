#![no_main]
#![no_std]

use arm_dcc::dprintln;
use panic_dcc as _;

#[rtfm::app(cores = 2)]
const APP: () = {
    #[init(core = 0, spawn = [foo])]
    fn init(c: init::Context) {
        #[global]
        static mut X: [u8; 1024] = [0; 1024];

        let x: &'static mut [u8; 1024] = X;

        assert!(x.as_ptr() as usize > 0x3_0000);

        dprintln!("init: OK");

        c.spawn.foo(x).ok().unwrap();
    }

    #[task(core = 1)]
    fn foo(_: foo::Context, x: &'static mut [u8; 1024]) {
        assert!(x.as_ptr() as usize > 0x3_0000);
        assert!(x.iter().all(|x| *x == 0));

        dprintln!("foo: OK");
    }
};
