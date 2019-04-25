pub fn nop() {
    unsafe { asm!("NOP" : : : : "volatile") }
}

pub fn wfi() {
    unsafe { asm!("WFI" : : : : "volatile") }
}
