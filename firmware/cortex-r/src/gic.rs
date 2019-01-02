use core::{
    fmt,
    marker::PhantomData,
    ops::Deref,
    sync::atomic::{AtomicBool, Ordering},
};

pub mod icc;
pub mod icd;

/// GIC Distributor registers
///
/// **IMPORTANT**: *Shared* between CPUs
pub struct ICD {
    // Make !Send and !Sync
    _0: PhantomData<*const ()>,
}

impl ICD {
    pub fn take() -> Option<Self> {
        /// FIXME not cross-core safe unless this static is placed in `.shared`
        static TAKEN: AtomicBool = AtomicBool::new(false);

        if TAKEN.compare_and_swap(false, true, Ordering::AcqRel) {
            None
        } else {
            Some(ICD { _0: PhantomData })
        }
    }

    pub unsafe fn steal() -> Self {
        ICD { _0: PhantomData }
    }

    pub fn disable(&mut self) {
        unsafe { self.ICDDCR.write(0) }
    }

    pub fn enable(&mut self) {
        unsafe { self.ICDDCR.write(1) }
    }

    pub fn unmask(n: u16) {
        unsafe {
            Self::steal().ICDISER[usize::from(n) / 32].write(1 << (n % 32));
        }
    }

    pub fn pend(n: u16) {
        unsafe {
            Self::steal().ICDISPR[usize::from(n) / 32].write(1 << (n % 32));
        }
    }

    pub fn icdsgir(target: Target, id: u8) {
        unsafe {
            let sgiintid = u32::from(id & 0b1111);

            let filter;
            let mut cpulist = 0;
            match target {
                Target::Loopback => filter = 0b10,
                Target::Broadcast => filter = 0b01,
                Target::Unicast(cpu) => {
                    filter = 0b00;
                    cpulist = 1 << (cpu & 0b111)
                }
            }

            // NOTE SATT = 0 sets the pending bit; SATT = 1 doesn't
            Self::steal().ICDSGIR.write(
                (filter << 24) /* TargetListFilter */ |
                (cpulist << 16) |
                (0 << 15) /* SATT */ |
                sgiintid,
            );
        }
    }

    pub unsafe fn set_priority(i: u16, priority: u8) {
        Self::steal().ICDIPR[usize::from(i)].write(priority)
    }
}

pub enum Target {
    // Anycast(u8),
    Broadcast,
    Loopback,
    Unicast(u8),
}

unsafe impl Send for ICD {}

impl Deref for ICD {
    type Target = icd::Registers;

    fn deref(&self) -> &icd::Registers {
        unsafe { &*(icd::BASE_ADDRESS as *const icd::Registers) }
    }
}

/// GIC CPU Interface registers
///
/// **IMPORTANT** One instance per CPU; all instances have the same base address
pub struct ICC {
    _0: PhantomData<*const ()>,
}

impl ICC {
    pub fn take() -> Option<Self> {
        static TAKEN: AtomicBool = AtomicBool::new(false);

        if TAKEN.compare_and_swap(false, true, Ordering::AcqRel) {
            None
        } else {
            Some(ICC { _0: PhantomData })
        }
    }

    pub unsafe fn steal() -> Self {
        ICC { _0: PhantomData }
    }

    pub fn disable(&mut self) {
        unsafe { self.ICCICR.write(0) }
    }

    pub fn get_icciar() -> ICCIAR {
        unsafe {
            ICCIAR {
                bits: Self::steal().ICCIAR.read(),
            }
        }
    }

    pub fn get_iccpmr() -> u8 {
        unsafe { Self::steal().ICCPMR.read() as u8 }
    }

    pub fn set_icceoir(icciar: ICCIAR) {
        unsafe { Self::steal().ICCEOIR.write(icciar.bits) }
    }

    pub unsafe fn set_iccpmr(threshold: u8) {
        asm!("" : : : "memory" : "volatile");
        Self::steal().ICCPMR.write(u32::from(threshold));
    }
}

#[derive(Clone, Copy)]
pub struct ICCIAR {
    bits: u32,
}

impl fmt::Debug for ICCIAR {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ICCIAR")
            .field("cpuid", &self.cpuid())
            .field("ackintid", &self.ackintid())
            .finish()
    }
}

impl ICCIAR {
    pub fn bits(&self) -> u32 {
        self.bits
    }

    pub fn ackintid(&self) -> u16 {
        (self.bits & ((1 << 10) - 1)) as u16
    }

    pub fn cpuid(&self) -> u8 {
        ((self.bits >> 10) & 0b111) as u8
    }
}

unsafe impl Send for ICC {}

impl Deref for ICC {
    type Target = icc::Registers;

    fn deref(&self) -> &icc::Registers {
        unsafe { &*(icc::BASE_ADDRESS as *const icc::Registers) }
    }
}

#[cfg(test)]
mod tests {
    use super::{icc, icd, ICC, ICD};

    #[test]
    fn offsets() {
        let icd = unsafe { ICD::steal() };

        assert_eq!(&icd.ICDDCR as *const _ as usize, icd::BASE_ADDRESS + 0x000);
        assert_eq!(&icd.ICDICTR as *const _ as usize, icd::BASE_ADDRESS + 0x004);
        assert_eq!(&icd.ICDIIDR as *const _ as usize, icd::BASE_ADDRESS + 0x008);

        assert_eq!(&icd.ICDISR as *const _ as usize, icd::BASE_ADDRESS + 0x080);
        assert_eq!(
            icd.ICDISR.last().unwrap() as *const _ as usize,
            icd::BASE_ADDRESS + 0x0FC
        );

        assert_eq!(&icd.ICDISER as *const _ as usize, icd::BASE_ADDRESS + 0x100);
        assert_eq!(
            icd.ICDISER.last().unwrap() as *const _ as usize,
            icd::BASE_ADDRESS + 0x17C
        );

        assert_eq!(&icd.ICDICER as *const _ as usize, icd::BASE_ADDRESS + 0x180);
        assert_eq!(
            icd.ICDICER.last().unwrap() as *const _ as usize,
            icd::BASE_ADDRESS + 0x1FC
        );

        assert_eq!(&icd.ICDISPR as *const _ as usize, icd::BASE_ADDRESS + 0x200);
        assert_eq!(
            icd.ICDISPR.last().unwrap() as *const _ as usize,
            icd::BASE_ADDRESS + 0x27C
        );

        assert_eq!(&icd.ICDICPR as *const _ as usize, icd::BASE_ADDRESS + 0x280);
        assert_eq!(
            icd.ICDICPR.last().unwrap() as *const _ as usize,
            icd::BASE_ADDRESS + 0x2FC
        );

        assert_eq!(&icd.ICDABR as *const _ as usize, icd::BASE_ADDRESS + 0x300);
        assert_eq!(
            icd.ICDABR.last().unwrap() as *const _ as usize,
            icd::BASE_ADDRESS + 0x37C
        );

        assert_eq!(&icd.ICDIPR as *const _ as usize, icd::BASE_ADDRESS + 0x400);
        assert!(icd.ICDIPR.last().unwrap() as *const _ as usize - icd::BASE_ADDRESS - 0x7F8 < 4);

        assert_eq!(
            &icd.ICDIPTR_ro as *const _ as usize,
            icd::BASE_ADDRESS + 0x800
        );
        assert!(
            icd.ICDIPTR_ro.last().unwrap() as *const _ as usize - icd::BASE_ADDRESS - 0x81C < 4
        );

        assert_eq!(
            &icd.ICDIPTR_rw as *const _ as usize,
            icd::BASE_ADDRESS + 0x820
        );
        assert!(
            icd.ICDIPTR_rw.last().unwrap() as *const _ as usize - icd::BASE_ADDRESS - 0xBF8 < 4
        );

        assert_eq!(&icd.ICDICFR as *const _ as usize, icd::BASE_ADDRESS + 0xC00);
        assert_eq!(
            icd.ICDICFR.last().unwrap() as *const _ as usize,
            icd::BASE_ADDRESS + 0xCFC
        );

        assert_eq!(&icd.ICDIDR as *const _ as usize, icd::BASE_ADDRESS + 0xD00);
        assert_eq!(
            icd.ICDIDR.last().unwrap() as *const _ as usize,
            icd::BASE_ADDRESS + 0xDFC
        );

        assert_eq!(&icd.ICDSGIR as *const _ as usize, icd::BASE_ADDRESS + 0xF00);

        assert_eq!(&icd.ICDIR as *const _ as usize, icd::BASE_ADDRESS + 0xFD0);
        assert_eq!(
            icd.ICDIR.last().unwrap() as *const _ as usize,
            icd::BASE_ADDRESS + 0xFFC
        );

        let icc = unsafe { ICC::steal() };

        assert_eq!(&icc.ICCICR as *const _ as usize, icc::BASE_ADDRESS + 0x00);
        assert_eq!(&icc.ICCPMR as *const _ as usize, icc::BASE_ADDRESS + 0x04);
        assert_eq!(&icc.ICCBPR as *const _ as usize, icc::BASE_ADDRESS + 0x08);
        assert_eq!(&icc.ICCIAR as *const _ as usize, icc::BASE_ADDRESS + 0x0C);
        assert_eq!(&icc.ICCEOIR as *const _ as usize, icc::BASE_ADDRESS + 0x10);
        assert_eq!(&icc.ICCRPR as *const _ as usize, icc::BASE_ADDRESS + 0x14);
        assert_eq!(&icc.ICCHPIR as *const _ as usize, icc::BASE_ADDRESS + 0x18);
        assert_eq!(&icc.ICCABPR as *const _ as usize, icc::BASE_ADDRESS + 0x1C);

        assert_eq!(&icc.ICCIDR as *const _ as usize, icc::BASE_ADDRESS + 0x40);
        assert_eq!(
            icc.ICCIDR.last().unwrap() as *const _ as usize,
            icc::BASE_ADDRESS + 0xCC
        );
    }
}
