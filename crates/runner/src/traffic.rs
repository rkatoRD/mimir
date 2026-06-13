use nr_core::{BearerId, Bits, SimRng, UeId};
use sap::{SlotContext, TrafficArrival, TrafficModel};

pub struct ConstantTraffic {
    bits_per_slot: u64,
}

impl ConstantTraffic {
    pub fn new(bits_per_slot: u64) -> Self {
        Self { bits_per_slot }
    }
}

impl TrafficModel for ConstantTraffic {
    fn generate(
        &mut self,
        _ctx: &SlotContext,
        ues: &[UeId],
        out: &mut Vec<TrafficArrival>,
        _rng: &mut SimRng,
    ) {
        for &ue in ues {
            out.push(TrafficArrival {
                ue,
                bearer: BearerId::new(0),
                size: Bits::new(self.bits_per_slot),
            });
        }
    }
}