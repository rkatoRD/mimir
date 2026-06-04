use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Direction {
    Downlink,
    Uplink,
}

impl fmt::Display for Direction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Direction::Downlink => write!(f, "DL"),
            Direction::Uplink => write!(f, "UL"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SlotDirection {
    Downlink,
    Uplink,
    Special,
}

impl SlotDirection {
    pub fn direction(&self) -> Option<Direction> {
        match self {
            SlotDirection::Downlink => Some(Direction::Downlink),
            SlotDirection::Uplink => Some(Direction::Uplink),
            SlotDirection::Special => None,
        }
    }
}

impl fmt::Display for SlotDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SlotDirection::Downlink => write!(f, "D"),
            SlotDirection::Uplink => write!(f, "U"),
            SlotDirection::Special => write!(f, "S"),
        }
    }
}
