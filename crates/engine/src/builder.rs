use nr_core::{Bits, CellId, Hz, Point, SimRng, UeId, Watt};
use nr_spec::Numerology;
use sap::{ChannelModel, Mac, Phy, SlotContext};

use crate::{
    cell_store::CellStore,
    clock::Clock,
    event_loop::EventLoop,
    ue_store::{UeState, UeStore},
};

struct CellSpec {
    id: CellId,
    position: Point,
    tx_power: Watt,
    mac: Box<dyn Mac>,
}

pub struct SimulatorBuilder<CH, PHY> {
    numerology: Numerology,
    bandwidth: Hz,
    total_prbs: u16,
    seed: u64,
    channel: CH,
    phy: PHY,
    cells: Vec<CellSpec>,
    ues: Vec<UeState>,
}

impl<CH, PHY> SimulatorBuilder<CH, PHY>
where
    CH: ChannelModel,
    PHY: Phy,
{
    pub fn new(
        numerology: Numerology,
        bandwidth: Hz,
        total_prbs: u16,
        seed: u64,
        channel: CH,
        phy: PHY,
    ) -> Self {
        Self {
            numerology,
            bandwidth,
            total_prbs,
            seed,
            channel,
            phy,
            cells: Vec::new(),
            ues: Vec::new(),
        }
    }

    pub fn add_cell(
        mut self,
        id: CellId,
        position: Point,
        tx_power: Watt,
        mac: Box<dyn Mac>,
    ) -> Self {
        self.cells.push(CellSpec {
            id,
            position,
            tx_power,
            mac,
        });
        self
    }

    pub fn add_ue(mut self, id: UeId, serving_cell: CellId, position: Point) -> Self {
        self.ues.push(UeState {
            id,
            serving_cell,
            position,
            backlog: Bits::ZERO,
        });
        self
    }

    pub fn build(self) -> EventLoop<CH, PHY> {
        let clock = Clock::new(self.numerology);

        let mut cells = CellStore::new();
        for spec in self.cells {
            cells.push(spec.id, spec.position, spec.tx_power, spec.mac);
        }

        let mut ues = UeStore::with_capacity(self.ues.len());
        for state in self.ues {
            ues.spawn(state);
        }

        let rng = SimRng::from_seed(self.seed);
        let slot_ctx = SlotContext {
            sfn_slot: clock.sfn_slot(),
            elapsed: clock.elapsed_slots(),
            bandwidth: self.bandwidth,
            total_prbs: self.total_prbs,
        };

        EventLoop::new(clock, cells, ues, self.channel, self.phy, rng, slot_ctx)
    }
}
