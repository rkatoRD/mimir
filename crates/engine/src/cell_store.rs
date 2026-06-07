use nr_core::{CellId, Point, Watt};
use sap::Mac;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CellSlot(u32);

impl CellSlot {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Default)]
pub struct CellStore {
    ids: Vec<CellId>,
    positions: Vec<Point>,
    tx_powers: Vec<Watt>,
    macs: Vec<Box<dyn Mac>>,
}

impl CellStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }

    pub fn push(
        &mut self,
        id: CellId,
        position: Point,
        tx_power: Watt,
        mac: Box<dyn Mac>,
    ) -> CellSlot {
        let slot = CellSlot(self.ids.len() as u32);
        self.ids.push(id);
        self.positions.push(position);
        self.tx_powers.push(tx_power);
        self.macs.push(mac);
        slot
    }

    pub fn id(&self, slot: CellSlot) -> CellId {
        self.ids[slot.index()]
    }

    pub fn position(&self, slot: CellSlot) -> Point {
        self.positions[slot.index()]
    }

    pub fn tx_power(&self, slot: CellSlot) -> Watt {
        self.tx_powers[slot.index()]
    }

    pub fn set_tx_power(&mut self, slot: CellSlot, power: Watt) {
        self.tx_powers[slot.index()] = power;
    }

    pub fn mac_mut(&mut self, slot: CellSlot) -> &mut dyn Mac {
        self.macs[slot.index()].as_mut()
    }

    pub fn iter_slots(&self) -> impl Iterator<Item = CellSlot> + '_ {
        (0..self.ids.len() as u32).map(CellSlot)
    }

    pub fn geometry(&self) -> (&[CellId], &[Point], &[Watt]) {
        (&self.ids, &self.positions, &self.tx_powers)
    }
}
