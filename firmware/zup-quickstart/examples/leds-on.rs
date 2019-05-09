//! Turns on the 4 user LEDs on the Ultra96 board (PS_MIO[17..20])

#![no_main]
#![no_std]

#[cfg(not(debug_assertions))]
use core::sync::atomic::{self, Ordering};

use panic_dcc as _;
use zup_rt::entry;

#[entry]
fn main() -> ! {
    let p = unsafe { zup::Peripherals::steal() };

    let mask = 0b1111u16 << 1; // [17:20]

    // configure pins as output
    p.GPIO.dirm_0.modify(|r, w| unsafe {
        w.direction_0()
            .bits(r.direction_0().bits() | ((mask as u32) << 16))
    });
    // enable output
    p.GPIO.oen_0.modify(|r, w| unsafe {
        w.op_enable_0()
            .bits(r.op_enable_0().bits() | ((mask as u32) << 16))
    });
    // set pins to 1
    p.GPIO
        .mask_data_0_msw
        .write(|w| unsafe { w.data_0_msw().bits(mask).mask_0_msw().bits(!mask) });

    loop {
        #[cfg(not(debug_assertions))]
        atomic::compiler_fence(Ordering::SeqCst);
    }
}
