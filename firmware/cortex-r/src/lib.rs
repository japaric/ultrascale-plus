#![feature(asm)]
#![no_std]

pub mod asm;
pub mod gic;
pub mod register;

// NOTE(unsafe) can break a critical section
pub unsafe fn enable_fiq() {
    match () {
        #[cfg(target_arch = "arm")]
        () => asm!("cpsie f" : : : "memory" : "volatile"),

        #[cfg(not(target_arch = "arm"))]
        () => unimplemented!(),
    }
}

pub fn disable_fiq() {
    match () {
        #[cfg(target_arch = "arm")]
        () => unsafe { asm!("cpsid f" : : : "memory" : "volatile") },

        #[cfg(not(target_arch = "arm"))]
        () => unimplemented!(),
    }
}

// NOTE(unsafe) can break a critical section
pub unsafe fn enable_irq() {
    match () {
        #[cfg(target_arch = "arm")]
        () => asm!("cpsie i" : : : "memory" : "volatile"),

        #[cfg(not(target_arch = "arm"))]
        () => unimplemented!(),
    }
}

pub fn disable_irq() {
    match () {
        #[cfg(target_arch = "arm")]
        () => unsafe { asm!("cpsid i" : : : "memory" : "volatile") },

        #[cfg(not(target_arch = "arm"))]
        () => unimplemented!(),
    }
}
