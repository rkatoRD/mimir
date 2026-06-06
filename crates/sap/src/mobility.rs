use crate::messages::SlotContext;
use nr_core::{Point, SimRng, UeId};

pub trait MobilityModel {
    fn step(
        &mut self,
        ctx: &SlotContext,
        ues: &[UeId],
        current: &[Point],
        out: &mut [Point],
        rng: &mut SimRng,
    );
}
