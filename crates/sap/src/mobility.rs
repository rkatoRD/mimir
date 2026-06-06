use crate::messages::SlotContext;
use nr_core::{Point, SimRng, UeId};

pub trait MobilityModel {
    fn next_position(
        &mut self,
        ctx: &SlotContext,
        ues: &[UeId],
        current: &[Point],
        out: &mut [Point],
        rng: &mut SimRng,
    );
}
