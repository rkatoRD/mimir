use crate::messages::SlotContext;
use nr_core::{CellId, Point, SimRng, UeId, Watt};

pub trait ChannelModel {
    fn update(&mut self, ctx: &SlotContext, rng: &mut SimRng);

    fn rx_power(
        &self,
        from: CellId,
        to: UeId,
        tx_power: Watt,
        tx_pos: Point,
        rx_pos: Point,
    ) -> Watt;
}
