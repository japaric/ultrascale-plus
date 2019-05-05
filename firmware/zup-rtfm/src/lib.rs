//! Multi-core Real Time For the Masses (RTFM), UltraScale+ edition

#![deny(missing_docs)]
#![deny(warnings)]
#![feature(maybe_uninit)]
#![feature(optin_builtin_traits)]
#![no_std]

use core::{fmt, ops};

pub use zup_rtfm_macros::app;

#[doc(hidden)]
pub mod export;

/// core-local data, a `&'static mut` reference tied to a particular core
#[derive(Eq, Ord, Hash, PartialEq, PartialOrd)]
pub struct Local<T>
where
    T: 'static,
{
    inner: &'static mut T,
}

impl<T> Local<T> {
    /// Pins the reference to this core
    pub fn pin(p: &'static mut T) -> Self {
        debug_assert!(
            (p as *mut T as usize) < 0x3_0000,
            "can't pin pointer {:?}; it doesn't point into the aliased TCM",
            p as *mut T,
        );

        Local { inner: p }
    }

    /// Grants temporary access to the core-local data
    pub fn borrow<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        f(self.inner)
    }

    /// Grants temporary access to the core-local data
    pub fn borrow_mut<R>(&mut self, f: impl FnOnce(&mut T) -> R) -> R {
        f(self.inner)
    }
}

/// A type that's safe to send across tasks running *on the same core*
pub auto trait LocalSend {}

// Types that are cross-core Send are also local-core Send
impl<T> LocalSend for T where T: Send {}

// `Local<T>` is local-core Send if the inner type is cross-core Send
impl<T> LocalSend for Local<T> where &'static mut T: Send {}

// `Local<T>` is tied to a core and can't be send to a different core
impl<T> !Send for Local<T> {}

/// A measurement of a monotonically nondecreasing clock. Opaque and useful only with `Duration`.
#[derive(Clone, Copy)]
pub struct Instant(i32);

impl Instant {
    /// Returns an instant corresponding to "now".
    pub fn now() -> Instant {
        unsafe { Instant((*export::TTC0::ptr()).counter_value_1.read().bits() as i32) }
    }

    /// Returns the amount of time elapsed from another instant to this one, or None if that instant
    /// is earlier than this one.
    pub fn checked_duration_since(&self, earlier: Instant) -> Option<Duration> {
        let diff = self.0 - earlier.0;

        if diff >= 0 {
            Some(Duration(diff as u32))
        } else {
            None
        }
    }

    /// Returns the amount of time elapsed from another instant to this one.
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

/// A `Duration` type to represent a span of time, typically used for system timeouts.
pub struct Duration(u32);

impl Duration {
    /// Returns the total number of clock cycles contained by this `Duration`
    pub fn as_cycles(&self) -> u32 {
        self.0
    }
}

/// Memory safe access to shared resources
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
