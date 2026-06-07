use nr_core::{CellId, Point, SimRng, UeId, Watt};
use sap::{
    ChannelModel, CoordinationMessage, Grant, Phy, PrbAllocation, SinrContext, SlotContext,
    TransportResult,
};

use crate::{
    cell_store::{CellSlot, CellStore},
    clock::Clock,
    ue_store::{UeSlot, UeState, UeStore},
};

#[derive(Debug, Clone)]
pub enum Directive {
    SetTxPower { cell: CellId, power: Watt },
    MutePrbs { cell: CellId, prbs: PrbAllocation },
}

pub struct EventLoop<CH, PHY> {
    clock: Clock,
    cells: CellStore,
    ues: UeStore,
    channel: CH,
    phy: PHY,
    rng: SimRng,
    slot_ctx: SlotContext,

    muted_prbs: Vec<PrbAllocation>,

    grant_buf: Vec<Grant>,
    result_buf: Vec<TransportResult>,
}

impl<CH, PHY> EventLoop<CH, PHY>
where
    CH: ChannelModel,
    PHY: Phy,
{
    pub(crate) fn new(
        clock: Clock,
        cells: CellStore,
        ues: UeStore,
        channel: CH,
        phy: PHY,
        rng: SimRng,
        slot_ctx: SlotContext,
    ) -> Self {
        let muted_prbs = vec![PrbAllocation::new(0, 0); cells.len()];
        Self {
            clock,
            cells,
            ues,
            channel,
            phy,
            rng,
            slot_ctx,
            muted_prbs,
            grant_buf: Vec::new(),
            result_buf: Vec::new(),
        }
    }

    pub fn clock(&self) -> &Clock {
        &self.clock
    }

    pub fn ues(&self) -> &UeStore {
        &self.ues
    }

    pub fn cells(&self) -> &CellStore {
        &self.cells
    }

    pub fn spawn_ue(&mut self, state: UeState) -> UeSlot {
        self.ues.spawn(state)
    }

    pub fn despawn_ue(&mut self, slot: UeSlot) -> bool {
        self.ues.despawn(slot)
    }

    pub fn cell_load_reports(&self) -> Vec<CoordinationMessage> {
        self.cells
            .iter_slots()
            .map(|slot| {
                let cell = self.cells.id(slot);
                let active_ues = self.active_ues_of(cell);
                CoordinationMessage::LoadReport {
                    cell,
                    active_ues,
                    used_prbs: self.muted_prbs[slot.index()].len(),
                }
            })
            .collect()
    }

    pub fn apply_directives(&mut self, directives: &[Directive]) {
        for directive in directives {
            match *directive {
                Directive::SetTxPower { cell, power } => {
                    if let Some(slot) = self.cell_slot_of(cell) {
                        self.cells.set_tx_power(slot, power);
                    }
                }
                Directive::MutePrbs { cell, prbs } => {
                    if let Some(slot) = self.cell_slot_of(cell) {
                        self.muted_prbs[slot.index()] = prbs;
                    }
                }
            }
        }
    }

    pub fn enqueue_traffic(
        &mut self,
        cell: CellId,
        arrivals: &[sap::messages::TrafficArrival],
    ) -> bool {
        let Some(slot) = self.cell_slot_of(cell) else {
            return false;
        };
        self.cells.mac_mut(slot).enqueue(arrivals);
        true
    }

    pub fn step(&mut self) {
        self.slot_ctx.sfn_slot = self.clock.sfn_slot();

        self.channel.update(&self.slot_ctx, &mut self.rng);

        let cell_slots: Vec<CellSlot> = self.cells.iter_slots().collect();
        for cell_slot in cell_slots {
            self.step_cell(cell_slot);
        }

        self.clock.tick();
    }

    fn step_cell(&mut self, cell_slot: CellSlot) {
        self.grant_buf.clear();
        self.cells
            .mac_mut(cell_slot)
            .step(&self.slot_ctx, &mut self.grant_buf);

        self.result_buf.clear();
        let serving_cell = self.cells.id(cell_slot);
        for gi in 0..self.grant_buf.len() {
            let grant = self.grant_buf[gi];
            let Some(rx_pos) = self
                .find_ue_slot(grant.ue)
                .and_then(|s| self.ues.get(s))
                .map(|s| s.position)
            else {
                continue;
            };
            let sinr = self.build_sinr(serving_cell, cell_slot, grant.ue, rx_pos);
            let result = self.phy.evaluate(&self.slot_ctx, &grant, &sinr);
            self.result_buf.push(result);
        }

        for ri in 0..self.result_buf.len() {
            let result = self.result_buf[ri];
            self.cells
                .mac_mut(cell_slot)
                .on_result(&self.slot_ctx, &result);
        }
    }

    fn build_sinr(
        &self,
        serving_cell: CellId,
        serving_slot: CellSlot,
        ue: UeId,
        rx_pos: Point,
    ) -> SinrContext {
        let serving = self.channel.rx_power(
            serving_cell,
            ue,
            self.cells.tx_power(serving_slot),
            self.cells.position(serving_slot),
            rx_pos,
        );

        let mut interference = Watt::new(0.0);
        for other in self.cells.iter_slots() {
            if other == serving_slot {
                continue;
            }
            let p = self.channel.rx_power(
                self.cells.id(other),
                ue,
                self.cells.tx_power(other),
                self.cells.position(other),
                rx_pos,
            );
            interference = interference + p;
        }

        SinrContext {
            ue,
            serving,
            interference,
            noise: self.thermal_noise(),
        }
    }

    fn thermal_noise(&self) -> Watt {
        const BOLTZMANN: f64 = 1.380_649e-23;
        const TEMPERATURE_K: f64 = 290.0;
        let bandwidth_hz = self.slot_ctx.bandwidth.value();
        Watt::new(BOLTZMANN * TEMPERATURE_K * bandwidth_hz)
    }

    fn find_ue_slot(&self, ue: UeId) -> Option<UeSlot> {
        self.ues
            .iter_slots()
            .find(|&slot| self.ues.id_of(slot) == Some(ue))
    }

    fn active_ues_of(&self, cell: CellId) -> u16 {
        self.ues
            .iter_slots()
            .filter(|&slot| self.ues.get(slot).is_some_and(|s| s.serving_cell == cell))
            .count() as u16
    }

    fn cell_slot_of(&self, cell: CellId) -> Option<CellSlot> {
        self.cells
            .iter_slots()
            .find(|&slot| self.cells.id(slot) == cell)
    }
}
