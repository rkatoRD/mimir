use crate::messages::{CoordinationMessage, Grant, SlotContext};

pub trait InterCellCoordinator {
    fn receive(&mut self, message: CoordinationMessage);

    fn emit(&mut self, ctx: &SlotContext, out: &mut Vec<CoordinationMessage>);

    fn coordinate(&mut self, ctx: &SlotContext, proposed: &[Grant], out: &mut Vec<Grant>);
}
