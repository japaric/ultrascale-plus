//! Receive an IPI from the APU

#![no_main]
#![no_std]

use core::{mem, ops, ptr};

use cortex_r::gic::{ICC, ICD};
use panic_dcc as _;
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
        icd.ICDIPTR_rw[usize::from(IPI_CH1) - 32].write(1 << 0);

        // set priority mask to the lowest priority
        icc.ICCPMR.write(248);

        // set the priority of IPI_CH1 to the second lowest priority
        icd.ICDIPR[usize::from(IPI_CH1)].write(240);

        // enable interrupt signaling
        icc.ICCICR
            .write((1 << 1) /* EnableNS */ | (1 << 0) /* EnableS */);

        // enable interrupt routing
        ICD::enable();

        // enable receiving interrupts from channel 0 (APU)
        ipi.ch1_ier.write(|w| w.ch0().set_bit());

        let msg = b"READY\n\0";
        TRACE[..msg.len()].copy_from_slice(msg);

        // IPI ourselves
        ipi.ch1_trig.write(|w| w.ch1().set_bit());

        // unmask IRQ
        cortex_r::enable_irq();

        loop {
            let isr = ipi.ch1_isr.read();
            if isr.ch0().bit_is_set() {
                // clear interrupt bit
                ipi.ch1_isr.write(|w| w.ch0().set_bit());

                let msg = b"RECEIVED IPI FROM CH0 (POLL)\n\0";
                TRACE[..msg.len()].copy_from_slice(msg);
            } else if isr.ch1().bit_is_set() {
                // clear interrupt bit
                ipi.ch1_isr.write(|w| w.ch1().set_bit());

                let msg = b"RECEIVED IPI FROM CH1 (POLL)\n\0";
                TRACE[..msg.len()].copy_from_slice(msg);
            }
        }
    }
}

#[interrupt]
fn IPI_CH1() {
    unsafe {
        let ipi = zup::Peripherals::steal().IPI;

        let isr = ipi.ch1_isr.read();
        if isr.ch0().bit_is_set() {
            // clear interrupt bit
            ipi.ch1_isr.write(|w| w.ch0().set_bit());

            let msg = b"RECEIVED IPI FROM CH0 (ISR)\n\0";
            TRACE[..msg.len()].copy_from_slice(msg);
        } else if isr.ch1().bit_is_set() {
            // clear interrupt bit
            ipi.ch1_isr.write(|w| w.ch1().set_bit());

            let msg = b"RECEIVED IPI FROM CH1 (ISR)\n\0";
            TRACE[..msg.len()].copy_from_slice(msg);
        }
    }
}

// NOTE unsynchronized access
struct Buffers0;

impl ops::Deref for Buffers0 {
    type Target = [Buffer; 8];

    fn deref(&self) -> &Self::Target {
        unsafe { &*(0xFF99_0000 as *const _) }
    }
}

impl ops::DerefMut for Buffers0 {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *(0xFF99_0000 as *mut _) }
    }
}

#[repr(C)]
struct Buffer {
    request: [u8; 32],
    response: [u8; 32],
}

impl Buffer {
    unsafe fn get_request<T>(&self) -> T
    where
        T: Copy,
    {
        assert!(mem::size_of::<T>() <= 32);
        assert!(mem::align_of::<T>() <= 4);

        ptr::read_volatile(&self.request as *const _ as *const T)
    }
}

// Trace buffer
#[repr(C)]
struct ResourceTable {
    // NOTE must be `1`
    ver: u32,
    num: u32,
    // NOTE must be `0`
    reserved: [u32; 2],
    offset: [u32; 1],
}

#[repr(C)]
struct TraceEntry {
    // NOTE must be `2`
    ty: u32,
    da: &'static [u8; 256],
    len: u32,
    // NOTE must be `0`
    reserved: u32,
    name: [u8; 32],
}

static mut TRACE: [u8; 256] = [0; 256];

#[link_section = ".resource_table"]
#[no_mangle]
static RESOURCES: Resources = Resources {
    table: ResourceTable {
        ver: 1,
        num: 1,
        reserved: [0; 2],
        offset: [mem::size_of::<ResourceTable>() as u32],
    },
    entries: [TraceEntry {
        ty: 2,
        da: unsafe { &TRACE },
        len: 256,
        reserved: 0,
        name: *b"trace0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
    }],
};

#[repr(C)]
struct Resources {
    table: ResourceTable,
    entries: [TraceEntry; 1],
}
