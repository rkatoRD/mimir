use serde::{Deserialize, Serialize};
use std::ops::{Add, Div, Mul, Sub};

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[repr(transparent)]
pub struct Hz(f64);

impl Hz {
    #[inline]
    pub const fn new(val: f64) -> Self {
        assert!(val.is_finite(), "Hz must be finite");
        assert!(val > 0.0, "Hz must be positive");
        Self(val)
    }

    #[inline]
    pub const fn value(self) -> f64 {
        self.0
    }

    #[inline]
    pub const fn to_mhz(self) -> f64 {
        self.0 / 1e6
    }

    #[inline]
    pub const fn to_ghz(self) -> f64 {
        self.0 / 1e9
    }
}

impl Add for Hz {
    type Output = Hz;
    #[inline]
    fn add(self, rhs: Self) -> Hz {
        Hz(self.0 + rhs.0)
    }
}

impl Sub for Hz {
    type Output = Hz;
    #[inline]
    fn sub(self, rhs: Self) -> Hz {
        Hz(self.0 - rhs.0)
    }
}

impl Mul<f64> for Hz {
    type Output = Hz;
    #[inline]
    fn mul(self, rhs: f64) -> Hz {
        Hz(self.0 * rhs)
    }
}

impl Div<f64> for Hz {
    type Output = Hz;
    #[inline]
    fn div(self, rhs: f64) -> Hz {
        Hz(self.0 / rhs)
    }
}
