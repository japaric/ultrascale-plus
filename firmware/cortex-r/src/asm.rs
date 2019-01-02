pub fn nop() {
    unsafe { asm!("NOP" : : : : "volatile") }
}
