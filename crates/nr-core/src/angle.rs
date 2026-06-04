use serde::{Deserialize, Serialize};
use std::ops::Sub;

use crate::geometry::Point;

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Rad(f64);

impl Rad {
    pub const fn new(val: f64) -> Self {
        assert!(val.is_finite(), "Rad must be finite");
        Self(val)
    }

    pub const fn value(self) -> f64 {
        self.0
    }

    pub fn atan2(y: f64, x: f64) -> Self {
        Self(y.atan2(x))
    }

    pub fn asin(val: f64) -> Self {
        Self(val.asin())
    }

    pub fn abs(self) -> Self {
        Self(self.0.abs())
    }

    pub fn sin(self) -> f64 {
        self.0.sin()
    }

    pub fn cos(self) -> f64 {
        self.0.cos()
    }

    /// normalize [-pi, pi]
    pub fn normalize(self) -> Self {
        Self(self.0.sin().atan2(self.0.cos()))
    }

    pub fn get_direction(position: Point) -> Rad {
        Rad::atan2(position.y.value(), position.x.value())
    }

    pub fn direction_between(from: &Point, to: &Point) -> Rad {
        Rad::atan2((to.y - from.y).value(), (to.x - from.x).value())
    }

    pub fn angle_diff(self, other: Rad) -> Rad {
        (self - other).normalize()
    }
}

impl Sub for Rad {
    type Output = Rad;
    fn sub(self, rhs: Rad) -> Rad {
        Rad(self.0 - rhs.0)
    }
}
