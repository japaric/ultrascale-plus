#![no_main]
#![no_std]

use core::sync::atomic::{AtomicBool, Ordering};

use arm_dcc::dprintln;
use microamp::shared;
use panic_dcc as _; // panic handler
use spin::Mutex; // spin = "0.5.0"
use zup_rt::entry;

#[shared]
static CHANNEL: Mutex<Option<&'static mut [u8; 1024]>> = Mutex::new(None);

#[shared]
static READY: AtomicBool = AtomicBool::new(false);

// runs on first core
#[cfg(core = "0")]
#[entry]
fn main() -> ! {
    static mut BUFFER: [u8; 1024] = [0; 1024];

    dprintln!("BUFFER is located at address {:?}", BUFFER.as_ptr());

    // send message
    *CHANNEL.lock() = Some(BUFFER);

    // unblock core #1
    READY.store(true, Ordering::Release);

    loop {}
}

// runs on second core
#[cfg(core = "1")]
#[entry]
fn main() -> ! {
    // wait until we receive a message
    while !READY.load(Ordering::Acquire) {
        // spin wait
    }

    let buffer: &'static mut [u8; 1024] = CHANNEL.lock().take().unwrap();

    dprintln!("Received a buffer located at address {:?}", buffer.as_ptr());

    // is this sound?
    // let first = buffer[0];

    loop {}
}
