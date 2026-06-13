use crate::messages::SlotContext;
use nr_core::{CellId, Point, SimRng, UeId, Watt};

pub trait ChannelModel {
    fn update(&mut self, ctx: &SlotContext, rng: &mut SimRng) -> bool;

    fn rx_power(
        &self,
        from: CellId,
        to: UeId,
        tx_power: Watt,
        tx_pos: Point,
        rx_pos: Point,
    ) -> Watt;

    fn rx_power_batch(
        &self,
        from: CellId,
        tx_power: Watt,
        tx_pos: Point,
        ues: &[UeId],
        rx_pos: &[Point],
        out: &mut [Watt],
    ) {
        for i in 0..ues.len() {
            out[i] = self.rx_power(from, ues[i], tx_power, tx_pos, rx_pos[i]);
        }
    }
}
