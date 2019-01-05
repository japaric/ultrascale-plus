//! Peripheral access through `/dev/mem`

use std::{
    error::Error,
    fs::OpenOptions,
    mem, ops,
    os::unix::io::AsRawFd,
    ptr,
    sync::atomic::{AtomicBool, Ordering},
};

use nix::sys::mman::{self, MapFlags, ProtFlags};
use zup::{gpio, ipi};

pub struct GPIO(*mut gpio::RegisterBlock);

impl ops::Deref for GPIO {
    type Target = gpio::RegisterBlock;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.0 }
    }
}

impl GPIO {
    pub fn take() -> Result<Option<Self>, Box<Error>> {
        static ONCE: AtomicBool = AtomicBool::new(false);

        if ONCE.compare_and_swap(false, true, Ordering::AcqRel) {
            Ok(None)
        } else {
            let f = OpenOptions::new().read(true).write(true).open("/dev/mem")?;

            let gpio = unsafe {
                mman::mmap(
                    ptr::null_mut(),
                    mem::size_of::<gpio::RegisterBlock>(),
                    // SysconfVar::PAGE_SIZE as usize,
                    ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
                    MapFlags::MAP_SHARED,
                    f.as_raw_fd(),
                    zup::GPIO::ptr() as u32 as i64,
                )?
            };

            Ok(Some(GPIO(gpio as *mut _)))
        }
    }
}

pub struct IPI(*mut ipi::RegisterBlock);

impl ops::Deref for IPI {
    type Target = ipi::RegisterBlock;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.0 }
    }
}

impl IPI {
    pub fn take() -> Result<Option<Self>, Box<Error>> {
        static ONCE: AtomicBool = AtomicBool::new(false);

        if ONCE.compare_and_swap(false, true, Ordering::AcqRel) {
            Ok(None)
        } else {
            let f = OpenOptions::new().read(true).write(true).open("/dev/mem")?;

            let ipi = unsafe {
                mman::mmap(
                    ptr::null_mut(),
                    mem::size_of::<ipi::RegisterBlock>(),
                    // SysconfVar::PAGE_SIZE as usize,
                    ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
                    MapFlags::MAP_SHARED,
                    f.as_raw_fd(),
                    zup::IPI::ptr() as u32 as i64,
                )?
            };

            Ok(Some(IPI(ipi as *mut _)))
        }
    }
}

#[repr(C)]
pub struct Buffer {
    request: [u8; 32],
    response: [u8; 32],
}

/// IPI buffers
pub struct Buffers(*mut [Buffer; 8]);

impl Buffers {
    pub fn take() -> Result<Option<Self>, Box<Error>> {
        static ONCE: AtomicBool = AtomicBool::new(false);

        if ONCE.compare_and_swap(false, true, Ordering::AcqRel) {
            Ok(None)
        } else {
            let f = OpenOptions::new().read(true).write(true).open("/dev/mem")?;

            let buffers = unsafe {
                mman::mmap(
                    ptr::null_mut(),
                    mem::size_of::<[Buffer; 8]>(),
                    // SysconfVar::PAGE_SIZE as usize,
                    ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
                    MapFlags::MAP_SHARED,
                    f.as_raw_fd(),
                    0xFF990000,
                )?
            };

            Ok(Some(Buffers(buffers as *mut _)))
        }
    }
}

impl ops::Deref for Buffers {
    type Target = [Buffer; 8];

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.0 }
    }
}

impl ops::DerefMut for Buffers {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.0 }
    }
}

impl Buffer {
    pub fn set_request<T>(&mut self, value: T)
    where
        T: Copy,
    {
        unsafe {
            assert!(mem::size_of::<T>() <= 32);
            assert!(mem::align_of::<T>() <= 4);

            ptr::write_volatile(self.request.as_mut_ptr() as *mut T, value);
        }
    }

    pub fn set_response<T>(&mut self, value: T)
    where
        T: Copy,
    {
        unsafe {
            assert!(mem::size_of::<T>() <= 32);
            assert!(mem::align_of::<T>() <= 4);

            ptr::write_volatile(self.response.as_mut_ptr() as *mut T, value);
        }
    }

    pub unsafe fn get_request<T>(&self) -> T
    where
        T: Copy,
    {
        assert!(mem::size_of::<T>() <= 32);
        assert!(mem::align_of::<T>() <= 4);

        ptr::read_volatile(self.request.as_ptr() as *const T)
    }

    pub unsafe fn get_response<T>(&self) -> T
    where
        T: Copy,
    {
        assert!(mem::size_of::<T>() <= 32);
        assert!(mem::align_of::<T>() <= 4);

        ptr::read_volatile(self.response.as_ptr() as *const T)
    }
}
