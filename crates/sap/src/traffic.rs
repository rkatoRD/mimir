use crate::messages::{SlotContext, TrafficArrival};
use nr_core::SimRng;

pub trait TrafficModel {
    fn generate(&mut self, ctx: &SlotContext, rng: &mut SimRng) -> Vec<TrafficArrival>;
}
