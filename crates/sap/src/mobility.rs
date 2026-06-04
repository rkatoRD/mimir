use crate::messages::SlotContext;
use nr_core::{Point, SimRng, UeId};

pub trait MobilityModel {
    fn next_position(
        &mut self,
        ctx: &SlotContext,
        ue: UeId,
        current: Point,
        rng: &mut SimRng,
    ) -> Point;
}
