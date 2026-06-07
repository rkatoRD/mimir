use nr_core::{Second, SfnSlot, Slot};
use nr_spec::Numerology;

#[derive(Debug, Clone)]
pub struct Clock {
    numerology: Numerology,
    slots_per_frame: u8,
    slot_duration: Second,
    sfn_slot: SfnSlot,
    elapsed_slots: Slot,
}

impl Clock {
    pub fn new(numerology: Numerology) -> Self {
        let slots_per_frame = numerology.slots_per_frame() as u8;
        Self {
            numerology,
            slots_per_frame,
            slot_duration: numerology.slot_duration(),
            sfn_slot: SfnSlot::new(0, 0),
            elapsed_slots: Slot::ZERO,
        }
    }

    pub fn sfn_slot(&self) -> SfnSlot {
        self.sfn_slot
    }

    pub fn elapsed_slots(&self) -> Slot {
        self.elapsed_slots
    }

    pub fn elapsed_time(&self) -> Second {
        self.elapsed_slots.to_second(self.slot_duration)
    }

    pub fn slot_duration(&self) -> Second {
        self.slot_duration
    }

    pub fn slots_per_frame(&self) -> u8 {
        self.slots_per_frame
    }

    pub fn numerology(&self) -> Numerology {
        self.numerology
    }

    pub fn tick(&mut self) {
        self.sfn_slot = self.sfn_slot.advance(self.slots_per_frame);
        self.elapsed_slots = Slot::new(self.elapsed_slots.value() + 1);
    }
}
