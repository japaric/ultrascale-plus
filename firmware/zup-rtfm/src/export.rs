use core::cell::Cell;
pub use core::mem::MaybeUninit;

pub use cortex_r::{
    enable_irq,
    gic::{Target, ICC, ICD},
};
pub use heapless::consts;
use heapless::spsc::Queue;
pub use microamp::app as amp;
pub use zup_rt::{entry, interrupt};

pub type FreeQueue<N> = Queue<u8, N>;
pub type ReadyQueue<T, N> = Queue<(T, u8), N>;

const PRIORITY_BITS: u8 = 5;

#[inline(always)]
pub fn run<F>(f: F)
where
    F: FnOnce(),
{
    let initial = ICC::get_iccpmr();
    f();
    unsafe { ICC::set_iccpmr(initial) }
}

#[inline(always)]
pub unsafe fn claim<T, R, F>(ptr: *mut T, priority: &Cell<u8>, ceiling: u8, f: F) -> R
where
    F: FnOnce(&mut T) -> R,
{
    let current = priority.get();

    if priority.get() < ceiling {
        priority.set(ceiling);
        ICC::set_iccpmr(logical2hw(ceiling));
        let r = f(&mut *ptr);
        ICC::set_iccpmr(logical2hw(current));
        priority.set(current);
        r
    } else {
        f(&mut *ptr)
    }
}

pub fn sgi(n: u8, core: Option<u8>) {
    if let Some(core) = core {
        ICD::icdsgir(Target::Unicast(core), n);
    } else {
        ICD::icdsgir(Target::Loopback, n);
    }
}

#[inline]
fn logical2hw(logical: u8) -> u8 {
    ((1 << PRIORITY_BITS) - logical) << (8 - PRIORITY_BITS)
}
