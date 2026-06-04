use crate::messages::{CoordinationMessage, Grant, SlotContext};

pub trait InterCellCoordinator {
    fn receive(&mut self, message: CoordinationMessage);

    fn emit(&mut self, ctx: &SlotContext) -> Vec<CoordinationMessage>;

    fn coordinate(&mut self, ctx: &SlotContext, proposed: Vec<Grant>) -> Vec<Grant>;
}
