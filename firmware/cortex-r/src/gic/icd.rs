use volatile_register::{RO, RW, WO};

pub const BASE_ADDRESS: usize = 0xF9000000;

#[allow(non_snake_case)]
#[repr(C)]
pub struct Registers {
    // 0x000 - Distributor Control Register
    pub ICDDCR: RW<u32>,
    // 0x004 - Interrupt Controller Type Register
    pub ICDICTR: RO<u32>,
    // 0x008 - Distributor Implementer Identification Register
    pub ICDIIDR: RO<u32>,
    /// 0x00C - 0x07C
    _reserved0: [u32; 29],
    /// 0x080..=0x0FC - Interrupt Security Registers
    pub ICDISR: [RW<u32>; 32],
    /// 0x100..=0x17C - Interrupt Set-Enable Registers
    // NOTE "In a multiprocessor implementation, ICDISER0 is banked for each connected processor"
    pub ICDISER: [RW<u32>; 32],
    /// 0x180..=0x1FC - Interrupt Clear-Enable Registers
    pub ICDICER: [RW<u32>; 32],
    /// 0x200..=0x27C - Interrupt Set-Pending Registers
    pub ICDISPR: [RW<u32>; 32],
    /// 0x280..=0x2FC - Interrupt Clear-Pending Registers
    pub ICDICPR: [RW<u32>; 32],
    /// 0x300..=0x37C - Active Bit Registers
    pub ICDABR: [RO<u32>; 32],
    /// 0x380..=0x3FC - Reserved
    _reserved1: [u32; 32],
    /// 0x400..=0x7F8 - Interrupt Priority Registers
    pub ICDIPR: [RW<u8>; 1020],
    /// 0x7FC - Reserved
    _reserved2: u32,
    // NOTE "In a multiprocessor implementation, ICDIPTR0 to ICDIPTR7 (viewed as 32-bit registers)
    // are banked for each connected processor"
    /// 0x800..=0x81C - Interrupt Processor Targets Registers (RO)
    pub ICDIPTR_ro: [RO<u8>; 32],
    /// 0x820..=0xBF8 - Interrupt Processor Targets Registers (RW)
    pub ICDIPTR_rw: [RW<u8>; 988],
    /// 0xBFC - Reserved
    _reserved3: u32,
    /// 0xC00..=0xCFC - Interrupt Configuration Registers
    pub ICDICFR: [RW<u32>; 64],
    /// 0xD00..=0xDFC - Implementation Defined Registers
    pub ICDIDR: [RW<u32>; 64],
    /// 0xE00..=0xEFC - Reserved
    _reserved4: [u32; 64],
    /// 0xF00 - Software Generated Interrupt Register
    pub ICDSGIR: WO<u32>,
    /// 0xF04..=0xFCC - Reserved
    _reserved5: [u32; 51],
    /// 0xFD0..=0xFFC - Identification Registers
    pub ICDIR: [RO<u32>; 12],
}
