use crate::messages::{Grant, SlotContext};
use nr_core::{Bits, Db, UeId};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SchedulingRequest {
    pub ue: UeId,
    pub backlog: Bits,
    pub channel_quality: Db,
}

pub trait Scheduler {
    fn schedule(&mut self, ctx: &SlotContext, requests: &[SchedulingRequest], out: &mut Vec<Grant>);
}
