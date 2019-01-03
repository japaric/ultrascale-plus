#![feature(maybe_uninit)]
#![no_std]

use core::{fmt, ops};

pub use zup_rtfm_macros::app;

#[doc(hidden)]
pub mod export;

pub trait Mutex {
    /// Data protected by the mutex
    type T;

    /// Creates a critical section and grants temporary access to the protected data
    fn lock<R, F>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut Self::T) -> R;
}

impl<'a, M> Mutex for &'a mut M
where
    M: Mutex,
{
    type T = M::T;

    fn lock<R, F>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut Self::T) -> R,
    {
        (**self).lock(f)
    }
}

/// Newtype over `&'a mut T` that implements the `Mutex` trait
///
/// The `Mutex` implementation for this type is a no-op, no critical section is created
pub struct Exclusive<'a, T>(pub &'a mut T);

impl<'a, T> Mutex for Exclusive<'a, T> {
    type T = T;

    fn lock<R, F>(&mut self, f: F) -> R
    where
        F: FnOnce(&mut Self::T) -> R,
    {
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
