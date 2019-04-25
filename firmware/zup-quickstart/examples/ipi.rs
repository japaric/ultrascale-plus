//! Sends IPI (Inter-Processor Interrupt) to itself (core 0)
//!
//! Expected output:
//!
//! ```
//! IRQ(ICCIAR { cpuid: 0, ackintid: 65 })
//! IPI_CH1(src=CH1, 42)
//! ~IRQ(ICCIAR { cpuid: 0, ackintid: 65 })
//! ```

#![no_main]
#![no_std]

extern crate panic_dcc;

use core::{mem, ops, ptr};

use arm_dcc::dprintln;
use cortex_r::gic::{ICC, ICD};
use zup::IPI;
use zup_rt::{entry, interrupt};

#[entry]
fn main() -> ! {
    unsafe {
        const IPI_CH1: u16 = 65;

        let icd = ICD::steal();
        let icc = ICC::steal();
        let ipi = zup::Peripherals::steal().IPI;

        // disable interrupt routing and signaling during configuration
        ICD::disable();
        ICC::disable();

        // unmask SPI 65
        ICD::unmask(IPI_CH1);

        // route SPI 65 to R5#0
        icd.ICDIPTR_rw[usize::from(IPI_CH1) - 32].write(1);

        // set priority mask to the lowest priority
        icc.ICCPMR.write(248);

        // set the priority of IPI_CH1 to the second lowest priority
        icd.ICDIPR[usize::from(IPI_CH1)].write(240);

        // enable interrupt signaling
        icc.ICCICR
            .write((1 << 1) /* EnableNS */ | (1 << 0) /* EnableS */);

        // enable interrupt routing
        ICD::enable();

        // enable receiving interrupts from channel 1
        ipi.ch1_ier.write(|w| w.ch1().set_bit());

        // write message
        Buffers1[0].set_request(42);

        // trigger IPI
        ipi.ch1_trig.write(|w| w.ch1().set_bit());

        // unmask IRQ
        cortex_r::enable_irq();
    }
    loop {}
}

#[interrupt]
fn IPI_CH1() {
    unsafe {
        let ipi = &*IPI::ptr();

        let isr = ipi.ch1_isr.read();
        if isr.ch1().bit_is_set() {
            // clear interrupt bit
            ipi.ch1_isr.write(|w| w.ch1().set_bit());

            dprintln!("IPI_CH1(src=CH1, {})", Buffers1[0].get_request::<i32>())
        } else {
            unimplemented!()
        }
    }
}

// NOTE unsynchronized access
struct Buffers1;

impl ops::Deref for Buffers1 {
    type Target = [Buffer; 8];

    fn deref(&self) -> &Self::Target {
        unsafe { &*(0xFF99_0200 as *const _) }
    }
}

impl ops::DerefMut for Buffers1 {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *(0xFF99_0200 as *mut _) }
    }
}

#[repr(C)]
struct Buffer {
    request: [u8; 32],
    response: [u8; 32],
}

impl Buffer {
    fn set_request<T>(&mut self, value: T)
    where
        T: Copy,
    {
        unsafe {
            assert!(mem::size_of::<T>() <= 32);
            assert!(mem::align_of::<T>() <= 4);

            ptr::write_volatile(&mut self.request as *mut _ as *mut T, value);
        }
    }

    unsafe fn get_request<T>(&self) -> T
    where
        T: Copy,
    {
        assert!(mem::size_of::<T>() <= 32);
        assert!(mem::align_of::<T>() <= 4);

        ptr::read_volatile(&self.request as *const _ as *const T)
    }
}
