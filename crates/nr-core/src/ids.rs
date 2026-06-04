use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CellId(u32);

impl CellId {
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    pub const fn value(self) -> u32 {
        self.0
    }
}

impl fmt::Display for CellId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Cell-{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UeId(u32);

impl UeId {
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    pub const fn value(self) -> u32 {
        self.0
    }
}

impl fmt::Display for UeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "UE-{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BearerId(u8);

impl BearerId {
    pub const fn new(id: u8) -> Self {
        Self(id)
    }

    pub const fn value(self) -> u8 {
        self.0
    }
}

impl fmt::Display for BearerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Bearer-{}", self.0)
    }
}
