use nr_core::SimRng;

use crate::messages::{Grant, SinrContext, SlotContext, TransportResult};

pub trait Phy {
    fn evaluate(
        &mut self,
        ctx: &SlotContext,
        grant: &Grant,
        sinr: &SinrContext,
        rng: &mut SimRng,
    ) -> TransportResult;
}
