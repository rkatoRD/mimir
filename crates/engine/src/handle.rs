use nr_core::{Second, Slot};
use sap::{ChannelModel, Phy};

use crate::event_loop::EventLoop;

pub struct RunFactory<F> {
    make: F,
}

impl<F, CH, PHY> RunFactory<F>
where
    F: Fn(u64) -> EventLoop<CH, PHY> + Send + Sync,
    CH: ChannelModel,
    PHY: Phy,
{
    pub fn new(make: F) -> Self {
        Self { make }
    }

    pub fn spawn_run(&self, seed: u64) -> SimRun<CH, PHY> {
        SimRun::new((self.make)(seed))
    }
}

pub struct SimRun<CH, PHY> {
    event_loop: EventLoop<CH, PHY>,
}

impl<CH, PHY> SimRun<CH, PHY>
where
    CH: ChannelModel,
    PHY: Phy,
{
    pub fn new(event_loop: EventLoop<CH, PHY>) -> Self {
        Self { event_loop }
    }

    pub fn run_slots(&mut self, slots: u64) {
        for _ in 0..slots {
            self.event_loop.step();
        }
    }

    pub fn run_for(&mut self, duration: Second) {
        let target = duration.to_slots(self.event_loop.clock().slot_duration());
        self.run_until(target);
    }

    pub fn run_until(&mut self, target: Slot) {
        while self.event_loop.clock().elapsed_slots().value() < target.value() {
            self.event_loop.step();
        }
    }

    pub fn event_loop(&self) -> &EventLoop<CH, PHY> {
        &self.event_loop
    }

    pub fn event_loop_mut(&mut self) -> &mut EventLoop<CH, PHY> {
        &mut self.event_loop
    }

    pub fn into_event_loop(self) -> EventLoop<CH, PHY> {
        self.event_loop
    }
}
