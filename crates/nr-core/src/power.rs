use serde::{Deserialize, Serialize};
use std::ops::{Add, Div, Sub};

#[derive(Debug, Clone, Copy, PartialEq, Default, PartialOrd, Serialize, Deserialize)]
#[repr(transparent)]
pub struct Dbm(f64);

impl Dbm {
    #[inline]
    pub const fn new(val: f64) -> Self {
        assert!(val.is_finite(), "Dbm must be finite");
        Self(val)
    }

    #[inline]
    pub const fn value(self) -> f64 {
        self.0
    }

    #[inline]
    pub fn to_watt(self) -> Watt {
        Watt::new(10.0_f64.powf((self.0 - 30.0) / 10.0))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default, PartialOrd, Serialize, Deserialize)]
#[repr(transparent)]
pub struct Db(f64);

impl Db {
    #[inline]
    pub const fn new(val: f64) -> Self {
        assert!(val.is_finite(), "Db must be finite");
        Self(val)
    }

    #[inline]
    pub const fn value(self) -> f64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default, PartialOrd, Serialize, Deserialize)]
#[repr(transparent)]
pub struct Watt(f64);

impl Watt {
    #[inline]
    pub const fn new(val: f64) -> Self {
        assert!(val.is_finite(), "Watt must be finite");
        assert!(val >= 0.0, "Watt must be non-negative");
        Self(val)
    }

    #[inline]
    pub const fn value(self) -> f64 {
        self.0
    }

    #[inline]
    pub fn to_dbm(self) -> Dbm {
        Dbm::new(10_f64 * self.0.log10() + 30.0)
    }
}

/// dBm + dB = dBm
impl Add<Db> for Dbm {
    type Output = Dbm;
    #[inline]
    fn add(self, rhs: Db) -> Dbm {
        Dbm(self.0 + rhs.0)
    }
}

/// dBm - dB = dBm
impl Sub<Db> for Dbm {
    type Output = Dbm;
    #[inline]
    fn sub(self, rhs: Db) -> Dbm {
        Dbm(self.0 - rhs.0)
    }
}

/// dB + dB = dB
impl Add for Db {
    type Output = Db;
    #[inline]
    fn add(self, rhs: Db) -> Db {
        Db(self.0 + rhs.0)
    }
}

/// dB - dB = dB
impl Sub for Db {
    type Output = Db;
    #[inline]
    fn sub(self, rhs: Db) -> Db {
        Db(self.0 - rhs.0)
    }
}

/// Watt + Watt = Watt
impl Add for Watt {
    type Output = Watt;
    #[inline]
    fn add(self, rhs: Watt) -> Watt {
        Watt(self.0 + rhs.0)
    }
}

/// Watt / Watt = f64
impl Div for Watt {
    type Output = f64;
    #[inline]
    fn div(self, rhs: Watt) -> f64 {
        self.0 / rhs.0
    }
}
