use crate::messages::{Grant, SlotContext, TrafficArrival, TransportResult};

pub trait Mac {
    fn enqueue(&mut self, arrival: TrafficArrival);

    fn step(&mut self, ctx: &SlotContext) -> Vec<Grant>;

    fn on_result(&mut self, ctx: &SlotContext, result: &TransportResult);
}
