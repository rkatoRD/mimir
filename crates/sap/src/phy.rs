use nr_core::Db;

use crate::messages::{Grant, SinrContext, SlotContext, TransportResult};

pub trait Phy {
    fn evaluate(&mut self, ctx: &SlotContext, grant: &Grant, sinr: &SinrContext)
    -> TransportResult;

    fn block_error_rate(&self, effective_sinr: Db, mcs_index: u8) -> f64;
}
