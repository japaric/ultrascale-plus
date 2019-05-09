//! Trace buffer

#![no_main]
#![no_std]

use core::mem;

use panic_dcc as _;
use zup_rt::entry;

static mut BUFFER: [u8; 256] = [0; 256];

#[entry]
fn main() -> ! {
    let msg = b"Hello, world!\n\0";
    unsafe {
        BUFFER[..msg.len()].copy_from_slice(msg);
    }

    loop {}
}

// For details about the .resource_table layout see include/linux/remoteproc.h [1]
//
// [1]: https://github.com/torvalds/linux/blob/v4.9/include/linux/remoteproc.h#L72
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

#[repr(C)]
struct Resources {
    table: ResourceTable,
    entries: [TraceEntry; 1],
}

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
        da: unsafe { &BUFFER },
        len: 256,
        reserved: 0,
        name: *b"trace0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0",
    }],
};
