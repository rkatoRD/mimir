use crate::messages::{Grant, PacketCompletion, SlotContext, TrafficArrival, TransportResult};

pub trait Mac {
    fn enqueue(&mut self, ctx: &SlotContext, arrivals: &[TrafficArrival]);

    fn step(&mut self, ctx: &SlotContext, out: &mut Vec<Grant>);

    fn on_result(&mut self, ctx: &SlotContext, result: &TransportResult);

    fn drain_completions(&mut self, _out: &mut Vec<PacketCompletion>) {}
}
