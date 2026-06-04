use nr_core::Second;

pub const FRAME_DURATION: Second = Second::new(10e-3);

pub const SUBFRAME_DURATION: Second = Second::new(1e-3);

pub const SUBFRAME_PER_FRAME: u32 = 10;

pub const SFN_PERIOD: u32 = 1024;

pub const SUBCARRIERS_PER_RB: u32 = 12;

pub const SYMBOLS_PER_SLOT_NORMAL_CP: u8 = 14;

pub const SYMBOLS_PER_SLOT_EXTENDED_CP: u8 = 12;
