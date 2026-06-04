use serde::{Deserialize, Serialize};
use std::ops::{Add, Div, Mul, Sub};

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Hz(f64);

impl Hz {
    pub const fn new(val: f64) -> Self {
        assert!(val.is_finite(), "Hz must be finite");
        assert!(val > 0.0, "Hz must be positive");
        Self(val)
    }

    pub const fn value(self) -> f64 {
        self.0
    }

    pub const fn to_mhz(self) -> f64 {
        self.0 / 1e6
    }

    pub const fn to_ghz(self) -> f64 {
        self.0 / 1e9
    }
}

impl Add for Hz {
    type Output = Hz;
    fn add(self, rhs: Self) -> Hz {
        Hz(self.0 + rhs.0)
    }
}

impl Sub for Hz {
    type Output = Hz;
    fn sub(self, rhs: Self) -> Hz {
        Hz(self.0 - rhs.0)
    }
}

impl Mul<f64> for Hz {
    type Output = Hz;
    fn mul(self, rhs: f64) -> Hz {
        Hz(self.0 * rhs)
    }
}

impl Div<f64> for Hz {
    type Output = Hz;
    fn div(self, rhs: f64) -> Hz {
        Hz(self.0 / rhs)
    }
}
