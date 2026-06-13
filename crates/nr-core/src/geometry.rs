use serde::{Deserialize, Serialize};
use std::ops::{Add, Mul, Sub};

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[repr(transparent)]
pub struct Meter(f64);

impl Meter {
    #[inline]
    pub const fn new(val: f64) -> Self {
        assert!(val.is_finite(), "Meter must be finite");
        Self(val)
    }

    #[inline]
    pub const fn value(self) -> f64 {
        self.0
    }

    #[inline]
    pub const fn to_km(self) -> f64 {
        self.0 / 1e3
    }
}

/// Meter + Meter = Meter
impl Add for Meter {
    type Output = Meter;
    #[inline]
    fn add(self, rhs: Self) -> Meter {
        Meter(self.0 + rhs.0)
    }
}

/// Meter - Meter = Meter
impl Sub for Meter {
    type Output = Meter;
    #[inline]
    fn sub(self, rhs: Self) -> Meter {
        Meter(self.0 - rhs.0)
    }
}

/// Meter * f64 = Meter
impl Mul<f64> for Meter {
    type Output = Meter;
    #[inline]
    fn mul(self, rhs: f64) -> Meter {
        Meter(self.0 * rhs)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: Meter,
    pub y: Meter,
    pub z: Meter,
}

impl Point {
    #[inline]
    pub const fn new(x: Meter, y: Meter, z: Meter) -> Self {
        Self { x, y, z }
    }

    #[inline]
    pub fn distance_2d(&self, other: &Point) -> Meter {
        let dx = self.x.0 - other.x.0;
        let dy = self.y.0 - other.y.0;
        Meter((dx * dx + dy * dy).sqrt())
    }

    #[inline]
    pub fn distance_3d(&self, other: &Point) -> Meter {
        let dx = self.x.0 - other.x.0;
        let dy = self.y.0 - other.y.0;
        let dz = self.z.0 - other.z.0;
        Meter((dx * dx + dy * dy + dz * dz).sqrt())
    }

    #[inline]
    pub const fn height(&self) -> f64 {
        self.z.0
    }

    #[inline]
    pub fn translate(&mut self, dx: Meter, dy: Meter) {
        self.x = self.x + dx;
        self.y = self.y + dy;
    }
}
