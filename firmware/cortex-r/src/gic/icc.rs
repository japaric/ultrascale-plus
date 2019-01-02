use volatile_register::{RO, RW, WO};

pub const BASE_ADDRESS: usize = super::icd::BASE_ADDRESS + 0x1000;

#[allow(non_snake_case)]
#[repr(C)]
pub struct Registers {
    /// 0x00 - CPU Interface Control Register
    pub ICCICR: RW<u32>,
    /// 0x04 - Interrupt Priority Mask Register
    pub ICCPMR: RW<u32>,
    /// 0x08 - Binary Point Register
    pub ICCBPR: RW<u32>,
    /// 0x0C - Interrupt Acknowledge Register
    pub ICCIAR: RO<u32>,
    /// 0x10 - End of Interrupt Register
    pub ICCEOIR: WO<u32>,
    /// 0x14 - Running Priority Register
    pub ICCRPR: RO<u32>,
    /// 0x18 - Highest Pending Interrupt Register
    pub ICCHPIR: RO<u32>,
    /// 0x1C - Aliased Binary Point Register
    pub ICCABPR: RW<u32>,
    /// 0x20..=0x3C - Reserved
    _reserved0: [u32; 8],
    /// 0x40..=0xCF - Implementation Defined Registers
    pub ICCIDR: [RW<u32>; 36],
}
