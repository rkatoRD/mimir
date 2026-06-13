use serde::{Deserialize, Serialize};
use std::ops::Sub;

use crate::geometry::Point;

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
#[repr(transparent)]
pub struct Rad(f64);

impl Rad {
    #[inline]
    pub const fn new(val: f64) -> Self {
        assert!(val.is_finite(), "Rad must be finite");
        Self(val)
    }

    #[inline]
    pub const fn value(self) -> f64 {
        self.0
    }

    #[inline]
    pub fn atan2(y: f64, x: f64) -> Self {
        Self(y.atan2(x))
    }

    #[inline]
    pub fn asin(val: f64) -> Self {
        Self(val.asin())
    }

    #[inline]
    pub fn abs(self) -> Self {
        Self(self.0.abs())
    }

    #[inline]
    pub fn sin(self) -> f64 {
        self.0.sin()
    }

    #[inline]
    pub fn cos(self) -> f64 {
        self.0.cos()
    }

    /// normalize [-pi, pi]
    #[inline]
    pub fn normalize(self) -> Self {
        Self(self.0.sin().atan2(self.0.cos()))
    }

    #[inline]
    pub fn get_direction(position: Point) -> Rad {
        Rad::atan2(position.y.value(), position.x.value())
    }

    #[inline]
    pub fn direction_between(from: &Point, to: &Point) -> Rad {
        Rad::atan2((to.y - from.y).value(), (to.x - from.x).value())
    }

    #[inline]
    pub fn angle_diff(self, other: Rad) -> Rad {
        (self - other).normalize()
    }
}

impl Sub for Rad {
    type Output = Rad;
    #[inline]
    fn sub(self, rhs: Rad) -> Rad {
        Rad(self.0 - rhs.0)
    }
}
