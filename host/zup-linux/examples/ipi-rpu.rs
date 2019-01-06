//! Send a IPI to RPU0

use std::{env, error::Error};

use zup_linux::{Buffers, IPI};

const CHANNEL: usize = 1; // RPU0

fn main() -> Result<(), Box<Error>> {
    let request: u32 = env::args().nth(1).expect("request").parse()?;

    let ipi = IPI::take()?.unwrap();
    let mut buffers = Buffers::take()?.unwrap();

    buffers[CHANNEL].set_request(request);

    // send IPI to channel 1 (RPU0)
    println!("before: {}", ipi.ch0_obs.read().bits());
    ipi.ch0_trig.write(|w| w.ch1().set_bit());
    println!("after: {}", ipi.ch0_obs.read().bits());

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
