pub enum Mode {
    User,
    Fiq,
    Irq,
    Supervisor,
    Abort,
    Undefined,
    System,
    Unknown,
}

#[derive(Clone, Copy)]
pub struct Cpsr {
    bits: u32,
}

impl Cpsr {
    pub fn bits(self) -> u32 {
        self.bits
    }

    /// Is the E bit set?
    pub fn e(&self) -> bool {
        self.bits & (1 << 9) != 0
    }

    /// Is the A bit set?
    pub fn a(&self) -> bool {
        self.bits & (1 << 8) != 0
    }

    /// Is the I bit set?
    pub fn i(&self) -> bool {
        self.bits & (1 << 7) != 0
    }

    /// Is the F bit set?
    pub fn f(&self) -> bool {
        self.bits & (1 << 6) != 0
    }

    /// Is the T bit set?
    pub fn t(&self) -> bool {
        self.bits & (1 << 5) != 0
    }

    /// Returns the content of the M bits
    pub fn mode(&self) -> Mode {
        match self.bits & 0b11111 {
            0b10000 => Mode::User,
            0b10001 => Mode::Fiq,
            0b10010 => Mode::Irq,
            0b10011 => Mode::Supervisor,
            0b10111 => Mode::Abort,
            0b11011 => Mode::Undefined,
            0b11111 => Mode::System,
            _ => Mode::Unknown,
        }
    }
}

#[cfg(target_arch = "arm")]
pub fn read() -> Cpsr {
    let bits: u32;
    unsafe { asm!("mrs $0, CPSR" : "=r"(bits) : : : "volatile") }
    Cpsr { bits }
}

#[cfg(not(target_arch = "arm"))]
pub fn read() -> Cpsr {
    unimplemented!();
}
