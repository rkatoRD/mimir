use serde::{Deserialize, Serialize};
use std::ops::{Add, Div, Mul, Sub};

#[derive(Debug, Clone, Copy, PartialEq, Default, PartialOrd, Serialize, Deserialize)]
pub struct Second(f64);

impl Second {
    pub const fn new(val: f64) -> Self {
        assert!(
            val.is_finite() && val >= 0.0,
            "Second must be non-negative and finite"
        );
        Self(val)
    }

    pub const fn value(self) -> f64 {
        self.0
    }

    pub const fn to_millis(self) -> f64 {
        self.0 * 1000.0
    }

    pub fn to_slots(self, slot_duration: Second) -> Slot {
        Slot((self.0 / slot_duration.0).round() as u64)
    }
}

impl Add for Second {
    type Output = Second;
    fn add(self, rhs: Self) -> Self {
        Second(self.0 + rhs.0)
    }
}

impl Sub for Second {
    type Output = Second;
    fn sub(self, rhs: Self) -> Self {
        Second(self.0 - rhs.0)
    }
}

impl Mul<f64> for Second {
    type Output = Second;
    fn mul(self, rhs: f64) -> Self {
        Second(self.0 * rhs)
    }
}

impl Div<f64> for Second {
    type Output = Second;
    fn div(self, rhs: f64) -> Self {
        Second(self.0 / rhs)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default, PartialOrd, Serialize, Deserialize)]
pub struct Slot(u64);

impl Slot {
    pub const ZERO: Self = Self(0);

    pub const fn new(val: u64) -> Self {
        Self(val)
    }

    pub const fn value(self) -> u64 {
        self.0
    }

    pub fn to_second(self, slot_duration: Second) -> Second {
        Second(self.0 as f64 * slot_duration.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default, PartialOrd, Serialize, Deserialize)]
pub struct SfnSlot {
    pub sfn: u16,
    pub slot: u8,
}

impl SfnSlot {
    pub const fn new(sfn: u16, slot: u8) -> Self {
        assert!(sfn <= 1023, "SFN must be 0..=1023");
        Self { sfn, slot }
    }

    pub fn from_slot(abs_slot: Slot, slots_per_frame: u8) -> Self {
        let spf = slots_per_frame as u64;
        let frame_total = abs_slot.0 / spf;
        let sfn = (frame_total % 1024) as u16;
        let slot = (abs_slot.0 % spf) as u8;
        Self { sfn, slot }
    }

    pub fn advance(&self, slots_per_frame: u8) -> Self {
        let next_slot = self.slot + 1;
        if next_slot < slots_per_frame {
            Self {
                sfn: self.sfn,
                slot: next_slot,
            }
        } else {
            let next_sfn = (self.sfn + 1) % 1024;
            Self {
                sfn: next_sfn,
                slot: 0,
            }
        }
    }
}
