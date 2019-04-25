#![feature(maybe_uninit)]
#![no_std]

use core::{fmt, ops};

pub use zup_rtfm_macros::app;

#[doc(hidden)]
pub mod export;

pub struct Instant(i32);

impl Instant {
    pub fn now() -> Instant {
        unsafe { Instant((*export::TTC0::ptr()).counter_value_1.read().bits() as i32) }
    }

    pub fn checked_duration_since(&self, earlier: Instant) -> Option<Duration> {
        let diff = self.0 - earlier.0;

        if diff >= 0 {
            Some(Duration(diff as u32))
        } else {
            None
        }
    }

    pub fn duration_since(&self, earlier: Instant) -> Duration {
        self.checked_duration_since(earlier).unwrap()
    }
}

impl fmt::Debug for Instant {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("Instant").field(&self.0).finish()
    }
}

impl ops::Sub<Instant> for Instant {
    type Output = Duration;

    fn sub(self, other: Instant) -> Duration {
        self.duration_since(other)
    }
}

pub struct Duration(u32);

impl Duration {
    /// Returns the total number of clock cycles contained by this `Duration`
    pub fn as_cycles(&self) -> u32 {
        self.0
    }
}

pub trait Mutex {
    /// Data protected by the mutex
    type T;

    /// Creates a critical section and grants temporary access to the protected data
    fn lock<R>(&mut self, f: impl FnOnce(&mut Self::T) -> R) -> R;
}

impl<'a, M> Mutex for &'a mut M
where
    M: Mutex,
{
    type T = M::T;

    fn lock<R>(&mut self, f: impl FnOnce(&mut Self::T) -> R) -> R {
        (**self).lock(f)
    }
}

/// Newtype over `&'a mut T` that implements the `Mutex` trait
///
/// The `Mutex` implementation for this type is a no-op, no critical section is created
pub struct Exclusive<'a, T>(pub &'a mut T);

impl<'a, T> Mutex for Exclusive<'a, T> {
    type T = T;

    fn lock<R>(&mut self, f: impl FnOnce(&mut Self::T) -> R) -> R {
        f(self.0)
    }
}

impl<'a, T> fmt::Debug for Exclusive<'a, T>
where
    T: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        (**self).fmt(f)
    }
}

impl<'a, T> fmt::Display for Exclusive<'a, T>
where
    T: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        (**self).fmt(f)
    }
}

impl<'a, T> ops::Deref for Exclusive<'a, T> {
    type Target = T;

    fn deref(&self) -> &T {
        self.0
    }
}

impl<'a, T> ops::DerefMut for Exclusive<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.0
    }
}
