//! IPI (Inter-Processor Interrupt) exchange between RPU0 and RPU1
//!
//! NOTE RPU0 must start executing its program *after* RPU1 starts executing its own
//!
//! Expected output:
//!
//! ```
//! core #0
//! IRQ(ICCIAR { cpuid: 0, ackintid: 65 })
//! IPI_CH1(src=RPU1, response=0x1722)
//! ~IRQ(ICCIAR { cpuid: 0, ackintid: 65 })
//! ```
//!
//! ```
//! core #1
//! IRQ(ICCIAR { cpuid: 0, ackintid: 66 })
//! IPI_CH2(src=RPU0, request=0x2217)
//! ~IRQ(ICCIAR { cpuid: 0, ackintid: 66 })
//! ```

#![no_main]
#![no_std]

const IPI_CH1_NR: u16 = 65;
const IPI_CH2_NR: u16 = 66;

use core::{mem, ops, ptr};

use arm_dcc::dprintln;
use cortex_r::gic::{ICC, ICD};
use panic_dcc as _;
use zup::IPI;
use zup_rt::{entry, interrupt};

#[entry]
fn main() -> ! {
    unsafe {
        dprintln!("core #{}", if cfg!(core = "0") { 0 } else { 1 });

        let icc = ICC::steal();
        let ipi = zup::Peripherals::steal().IPI;

        // disable interrupt routing and signaling during configuration
        ICC::disable();

        // set priority mask to the lowest priority
        icc.ICCPMR.write(248);

        // enable interrupt signaling
        icc.ICCICR
            .write((1 << 1) /* EnableNS */ | (1 << 0) /* EnableS */);

        // the ICD peripheral is shared; make sure we initialize it just once
        if cfg!(core = "1") {
            let icd = ICD::steal();

            ICD::disable();

            // unmask SPI IPI_CH1
            ICD::unmask(IPI_CH1_NR);

            // unmask SPI IPI_CH2
            ICD::unmask(IPI_CH2_NR);

            // route SPI IPI_CH1 to R5#0
            icd.ICDIPTR_rw[usize::from(IPI_CH1_NR) - 32].write(1 << 0);

            // route SPI IPI_CH2 to R5#1
            icd.ICDIPTR_rw[usize::from(IPI_CH2_NR) - 32].write(1 << 1);

            // set the priority of IPI_CH{1,2} to the second lowest priority
            icd.ICDIPR[usize::from(IPI_CH1_NR)].write(240);
            icd.ICDIPR[usize::from(IPI_CH2_NR)].write(240);

            // enable interrupt routing
            ICD::enable();
        }

        if cfg!(core = "0") {
            // enable receiving interrupts from channel 2
            ipi.ch1_ier.write(|w| w.ch2().set_bit());

            // write request message
            BUFFERS.write_request(Agent::RPU0, Agent::RPU1, 0x2217);

            // send IPI to channel 2
            ipi.ch1_trig.write(|w| w.ch2().set_bit());
        } else {
            // enable receiving interrupts from channel 1
            ipi.ch2_ier.write(|w| w.ch1().set_bit());
        }

        // unmask IRQ
        cortex_r::enable_irq();

        loop {}
    }
}

#[cfg(core = "0")]
#[interrupt]
fn IPI_CH1() {
    unsafe {
        let ipi = &*IPI::ptr();

        let isr = ipi.ch1_isr.read();
        if isr.ch2().bit_is_set() {
            // clear interrupt bit
            ipi.ch1_isr.write(|w| w.ch2().set_bit());

            dprintln!(
                "IPI_CH1(src=RPU1, response={:#x})",
                BUFFERS.read_response::<i32>(Agent::RPU0, Agent::RPU1)
            );
        } else {
            dprintln!("IPI_CH1(isr={:#?})", isr.bits());
        }
    }
}

#[cfg(core = "1")]
#[interrupt]
fn IPI_CH2() {
    unsafe {
        let ipi = &*IPI::ptr();

        let isr = ipi.ch2_isr.read();
        if isr.ch1().bit_is_set() {
            // clear interrupt bit
            ipi.ch2_isr.write(|w| w.ch1().set_bit());

            dprintln!(
                "IPI_CH2(src=RPU0, request={:#x})",
                BUFFERS.read_request::<i32>(Agent::RPU1, Agent::RPU0)
            );

            // send a response
            // - write message
            BUFFERS.write_response(Agent::RPU1, Agent::RPU0, 0x1722);

            // - send IPI to channel 1
            ipi.ch2_trig.write(|w| w.ch1().set_bit());
        } else {
            unimplemented!()
        }
    }
}

// NOTE unsynchronized access
struct BUFFERS;

impl ops::Deref for BUFFERS {
    type Target = Buffers;

    fn deref(&self) -> &Self::Target {
        unsafe { &*(0xFF99_0000 as *const _) }
    }
}

impl ops::DerefMut for BUFFERS {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *(0xFF99_0000 as *mut _) }
    }
}

#[derive(Clone, Copy)]
enum Agent {
    // actually this is agent 2 (agent numbering starts at 0)
    RPU0 = 1,
    // actually this is agent 3 (agent numbering starts at 0)
    RPU1 = 2,
}

#[repr(transparent)]
struct Buffers([[Buffer; 8]; 8]);

impl Buffers {
    #[allow(dead_code)]
    unsafe fn read_response<T>(&self, i_am: Agent, requestee: Agent) -> T
    where
        T: Copy,
    {
        self.0[i_am as usize][requestee as usize].get_response()
    }

    #[allow(dead_code)]
    unsafe fn read_request<T>(&self, i_am: Agent, requester: Agent) -> T
    where
        T: Copy,
    {
        self.0[requester as usize][i_am as usize].get_request()
    }

    #[allow(dead_code)]
    fn write_response<T>(&mut self, i_am: Agent, requester: Agent, value: T)
    where
        T: Copy,
    {
        self.0[requester as usize][i_am as usize].set_response(value)
    }

    fn write_request<T>(&mut self, i_am: Agent, requestee: Agent, value: T)
    where
        T: Copy,
    {
        self.0[i_am as usize][requestee as usize].set_request(value)
    }
}

#[repr(C)]
#[derive(Debug)]
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

            ptr::write_volatile(self.request.as_mut_ptr() as *mut T, value);
        }
    }

    #[allow(dead_code)]
    fn set_response<T>(&mut self, value: T)
    where
        T: Copy,
    {
        unsafe {
            assert!(mem::size_of::<T>() <= 32);
            assert!(mem::align_of::<T>() <= 4);

            ptr::write_volatile(self.response.as_mut_ptr() as *mut T, value);
        }
    }

    #[allow(dead_code)]
    unsafe fn get_request<T>(&self) -> T
    where
        T: Copy,
    {
        assert!(mem::size_of::<T>() <= 32);
        assert!(mem::align_of::<T>() <= 4);

        ptr::read_volatile(self.request.as_ptr() as *const T)
    }

    #[allow(dead_code)]
    unsafe fn get_response<T>(&self) -> T
    where
        T: Copy,
    {
        assert!(mem::size_of::<T>() <= 32);
        assert!(mem::align_of::<T>() <= 4);

        ptr::read_volatile(self.response.as_ptr() as *const T)
    }
}
