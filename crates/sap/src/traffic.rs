use crate::messages::{SlotContext, TrafficArrival};
use nr_core::{SimRng, UeId};

pub trait TrafficModel {
    fn generate(
        &mut self,
        ctx: &SlotContext,
        ues: &[UeId],
        out: &mut Vec<TrafficArrival>,
        rng: &mut SimRng,
    );
}
