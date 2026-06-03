use serde::{Deserialize, Serialize};

use crate::time::Second;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimConfig {
    pub seed: u64,
    pub duration: Second,
    pub numerology: u8,
}

impl SimConfig {
    pub fn new(seed: u64, duration: Second, numerology: u8) -> Self {
        assert!(numerology <= 3, "numerology must be 0..=3");
        Self {
            seed,
            duration,
            numerology,
        }
    }

    pub fn slots_per_frame(&self) -> u8 {
        10 * (1 << self.numerology)
    }
}

impl Default for SimConfig {
    fn default() -> Self {
        Self::new(42, Second::new(1.0), 1)
    }
}
