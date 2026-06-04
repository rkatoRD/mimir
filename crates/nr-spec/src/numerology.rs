use nr_core::{Hz, Second};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Numerology(pub u8);

impl Numerology {
    pub const SYMBOLS_PER_SLOT: u8 = 14;

    pub const fn new(mu: u8) -> Self {
        assert!(mu <= 4, "numerology must be in 0..=4");
        Self(mu)
    }

    pub const fn scs(self) -> Hz {
        Hz::new(15_000.0 * (1u64 << self.0) as f64)
    }

    pub const fn slots_per_subframe(self) -> u32 {
        1 << self.0
    }

    pub const fn slots_per_frame(self) -> u32 {
        10 * self.slots_per_subframe()
    }

    pub fn slot_duration(self) -> Second {
        Second::new(1e-3 / (1 << self.0) as f64)
    }

    pub fn symbol_duration(self) -> Second {
        Second::new(1.0 / self.scs().value())
    }
}
