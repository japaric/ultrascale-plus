use core::cell::Cell;

pub use cortex_r::{
    enable_irq,
    gic::{Target, ICC, ICD},
};
use heapless::spsc::{MultiCore, SingleCore};
pub use heapless::{consts, i::Queue as iQueue, spsc::Queue};
pub use microamp::shared;
pub use zup::TTC0;
pub use zup_rt::Interrupt;

pub type MCFQ<N> = Queue<u8, N, u8, MultiCore>;
pub type MCRQ<T, N> = Queue<(T, u8), N, u8, MultiCore>;
pub type SCFQ<N> = Queue<u8, N, u8, SingleCore>;
pub type SCRQ<T, N> = Queue<(T, u8), N, u8, SingleCore>;

const PRIORITY_BITS: u8 = 5;
const NSGIS: u8 = 16;

pub struct Priority {
    inner: Cell<u8>,
}

impl Priority {
    #[inline(always)]
    pub unsafe fn new(value: u8) -> Self {
        Priority {
            inner: Cell::new(value),
        }
    }

    // these two methods are used by `lock` (see below) but can't be used from the RTFM application
    #[inline(always)]
    fn set(&self, value: u8) {
        self.inner.set(value)
    }

    #[inline(always)]
    fn get(&self) -> u8 {
        self.inner.get()
    }
}

#[inline(always)]
pub fn run(priority: u8, f: impl FnOnce()) {
    if priority == 1 {
        f();
        unsafe { ICC::set_iccpmr(!0) }
    } else {
        let initial = ICC::get_iccpmr();
        f();
        unsafe { ICC::set_iccpmr(initial) }
    }
}

#[inline(always)]
pub unsafe fn lock<T, R>(
    ptr: *mut T,
    priority: &Priority,
    ceiling: u8,
    f: impl FnOnce(&mut T) -> R,
) -> R {
    let current = priority.get();

    if priority.get() < ceiling {
        priority.set(ceiling);
        ICC::set_iccpmr(logical2hw(ceiling + 1));
        let r = f(&mut *ptr);
        ICC::set_iccpmr(logical2hw(current + 1));
        priority.set(current);
        r
    } else {
        f(&mut *ptr)
    }
}

pub fn clear_sgis() {
    unsafe { ICD::steal().ICDICPR[0].write((1 << NSGIS) - 1) }
}

pub fn sgi(n: u8, core: Option<u8>) {
    if let Some(core) = core {
        ICD::icdsgir(Target::Unicast(core), n);
    } else {
        ICD::icdsgir(Target::Loopback, n);
    }
}

#[inline]
pub fn logical2hw(logical: u8) -> u8 {
    ((1 << PRIORITY_BITS) - logical) << (8 - PRIORITY_BITS)
}

pub fn assert_send<T>()
where
    T: Send,
{
}

pub fn assert_local_send<T>()
where
    T: crate::LocalSend,
{
}

pub fn assert_sync<T>()
where
    T: Sync,
{
}

pub fn setup_counter() {
    unsafe {
        // set prescaler to 1
        (*TTC0::ptr()).clock_control_1.reset();
        // reset and start counter
        (*TTC0::ptr())
            .counter_control_1
            .write(|w| w.rst().set_bit().dis().clear_bit());
    }
}
