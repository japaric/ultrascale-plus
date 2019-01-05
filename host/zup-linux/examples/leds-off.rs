//! Turns off the 4 user LEDs on the Ultra96 board (PS_MIO[17..20])
//!
//! It's recommended that you set the `trigger` of each led (`/sys/class/leds/ds*`) to `none` or
//! `default-on` before using this program.

use std::error::Error;

use zup_linux::GPIO;

fn main() -> Result<(), Box<Error>> {
    let gpio = GPIO::take()?.unwrap();

    let mask = 0b1111u16 << 1; // [17:20]

    // configure pins as output
    gpio.dirm_0.modify(|r, w| unsafe {
        w.direction_0()
            .bits(r.direction_0().bits() | ((mask as u32) << 16))
    });

    // enable output
    gpio.oen_0.modify(|r, w| unsafe {
        w.op_enable_0()
            .bits(r.op_enable_0().bits() | ((mask as u32) << 16))
    });

    // set pins to 0
    gpio.mask_data_0_msw
        .write(|w| unsafe { w.data_0_msw().bits(0).mask_0_msw().bits(!mask) });

    Ok(())
}

// HACK to make linking work
#[no_mangle]
extern "C" fn __addtf3() {
    unimplemented!()
}

#[no_mangle]
extern "C" fn __multf3() {
    unimplemented!()
}

#[no_mangle]
extern "C" fn __subtf3() {
    unimplemented!()
}
