use std::{env, error::Error, fs::File, os::unix::io::AsRawFd, ptr};

use nix::{
    sys::mman::{self, MapFlags, ProtFlags},
    unistd::SysconfVar,
};

fn main() -> Result<(), Box<Error>> {
    let mut args = env::args();
    let mut addr = u32::from_str_radix(args.nth(1).expect("address").trim_start_matches("0x"), 16)?;
    let length = args.next().expect("length").parse()?;

    let f = File::open("/dev/mem")?;

    let ocm = unsafe {
        mman::mmap(
            ptr::null_mut(),
            SysconfVar::PAGE_SIZE as usize,
            ProtFlags::PROT_READ,
            MapFlags::MAP_SHARED,
            f.as_raw_fd(),
            addr as _,
        )? as *const u32
    };

    for i in 0..length {
        println!("{:#010X}: {:#010X}", addr, unsafe {
            ptr::read_volatile(ocm.add(i))
        });
        addr += 4;
    }

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
