use serde::{Deserialize, Serialize};
use std::ops::{Add, AddAssign, Div, Mul, Sub};

use crate::time::Second;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[repr(transparent)]
pub struct Bytes(u64);

impl Bytes {
    pub const ZERO: Self = Self(0);

    #[inline]
    pub const fn new(val: u64) -> Self {
        Self(val)
    }

    #[inline]
    pub const fn value(self) -> u64 {
        self.0
    }

    #[inline]
    pub const fn to_bits(self) -> Bits {
        Bits(self.0.checked_mul(8).expect("Bytes::to_bits overflow"))
    }
}

impl Add for Bytes {
    type Output = Bytes;
    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self(self.0.checked_add(rhs.0).expect("Bytes overflow"))
    }
}

impl AddAssign for Bytes {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl Sub for Bytes {
    type Output = Bytes;
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self(self.0.checked_sub(rhs.0).expect("Bytes underflow"))
    }
}

impl Mul<u64> for Bytes {
    type Output = Bytes;
    #[inline]
    fn mul(self, rhs: u64) -> Self {
        Self(self.0 * rhs)
    }
}

impl Div<u64> for Bytes {
    type Output = Bytes;
    #[inline]
    fn div(self, rhs: u64) -> Self {
        Self(self.0 / rhs)
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[repr(transparent)]
pub struct Bits(u64);

impl Bits {
    pub const ZERO: Self = Self(0);

    #[inline]
    pub const fn new(val: u64) -> Self {
        Self(val)
    }

    #[inline]
    pub const fn value(self) -> u64 {
        self.0
    }

    #[inline]
    pub const fn to_bytes(self) -> Bytes {
        assert!(self.0 % 8 == 0, "bits is not a multiple of 8");
        Bytes(self.0 / 8)
    }
}

impl Add for Bits {
    type Output = Bits;
    #[inline]
    fn add(self, rhs: Self) -> Self {
        Self(self.0.checked_add(rhs.0).expect("Bits overflow"))
    }
}

impl AddAssign for Bits {
    #[inline]
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl Sub for Bits {
    type Output = Bits;
    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self(self.0.checked_sub(rhs.0).expect("Bits underflow"))
    }
}

impl Mul<u64> for Bits {
    type Output = Bits;
    #[inline]
    fn mul(self, rhs: u64) -> Self {
        Self(self.0 * rhs)
    }
}

impl Div<u64> for Bits {
    type Output = Bits;
    #[inline]
    fn div(self, rhs: u64) -> Self {
        Self(self.0 / rhs)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[repr(transparent)]
pub struct Bps(f64);

impl Bps {
    pub const ZERO: Self = Self(0.0);

    #[inline]
    pub const fn new(val: f64) -> Self {
        Self(val)
    }

    #[inline]
    pub const fn value(self) -> f64 {
        self.0
    }
}

/// Bits / Second = Bps
impl Div<Second> for Bits {
    type Output = Bps;
    #[inline]
    fn div(self, rhs: Second) -> Bps {
        Bps(self.0 as f64 / rhs.value())
    }
}
