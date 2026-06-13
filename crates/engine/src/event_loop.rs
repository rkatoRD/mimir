use std::collections::HashMap;

use nr_core::{CellId, Point, SimRng, UeId, Watt};
use sap::{
    ChannelModel, CoordinationMessage, Grant, PacketCompletion, Phy, PrbAllocation, SinrContext,
    SlotContext, TrafficArrival, TrafficModel, TransportResult,
};

use crate::{
    cell_store::{CellSlot, CellStore},
    clock::Clock,
    radio_map::RadioMap,
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

    ue_index: HashMap<UeId, UeSlot>,
    channel: CH,
    phy: PHY,

    rng: SimRng,
    slot_ctx: SlotContext,

    radio_map: RadioMap,
    thermal_noise: Watt,

    muted_prbs: Vec<PrbAllocation>,
    used_prbs: Vec<u16>,

    grant_buf: Vec<Grant>,
    result_buf: Vec<TransportResult>,
    arrival_buf: Vec<TrafficArrival>,
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
        let n_cells = cells.len();
        let muted_prbs = vec![PrbAllocation::new(0, 0); n_cells];
        let used_prbs = vec![0u16; n_cells];

        let mut ue_index = HashMap::with_capacity(ues.array_len());
        for slot in ues.iter_slots() {
            if let Some(id) = ues.id_of(slot) {
                ue_index.insert(id, slot);
            }
        }

        let radio_map = RadioMap::with_capacity(n_cells, ues.array_len());
        let thermal_noise = thermal_noise_for(&slot_ctx);

        Self {
            clock,
            cells,
            ues,
            ue_index,
            channel,
            phy,
            rng,
            slot_ctx,
            radio_map,
            thermal_noise,
            muted_prbs,
            used_prbs,
            grant_buf: Vec::new(),
            result_buf: Vec::new(),
            arrival_buf: Vec::new(),
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

    pub fn last_results(&self) -> &[TransportResult] {
        &self.result_buf
    }

    pub fn spawn_ue(&mut self, state: UeState) -> UeSlot {
        let slot = self.ues.spawn(state);
        self.ue_index.insert(state.id, slot);
        self.radio_map.mark_dirty();
        slot
    }

    pub fn despawn_ue(&mut self, slot: UeSlot) -> bool {
        let Some(id) = self.ues.id_of(slot) else {
            return false;
        };
        let removed = self.ues.despawn(slot);
        if removed {
            self.ue_index.remove(&id);
            self.radio_map.mark_dirty();
        }
        removed
    }

    pub fn set_ue_position(&mut self, slot: UeSlot, position: Point) -> bool {
        let updated = self.ues.set_position(slot, position);
        if updated {
            self.radio_map.mark_dirty();
        }
        updated
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
                    used_prbs: self.used_prbs[slot.index()],
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
                        self.radio_map.mark_dirty();
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
        self.cells.mac_mut(slot).enqueue(&self.slot_ctx, arrivals);
        true
    }

    pub fn generate_and_enqueue_traffic(
        &mut self,
        cell: CellId,
        traffic: &mut dyn TrafficModel,
        ues: &[UeId],
    ) -> bool {
        let Some(slot) = self.cell_slot_of(cell) else {
            return false;
        };
        self.arrival_buf.clear();
        traffic.generate(&self.slot_ctx, ues, &mut self.arrival_buf, &mut self.rng);
        self.cells
            .mac_mut(slot)
            .enqueue(&self.slot_ctx, &self.arrival_buf);
        true
    }

    pub fn drain_completions(&mut self, out: &mut Vec<PacketCompletion>) {
        for i in 0..self.cells.len() {
            self.cells
                .mac_mut(CellSlot::from_index(i))
                .drain_completions(out);
        }
    }

    pub fn step(&mut self) {
        self.slot_ctx.sfn_slot = self.clock.sfn_slot();
        self.slot_ctx.elapsed = self.clock.elapsed_slots();

        if self.channel.update(&self.slot_ctx, &mut self.rng) {
            self.radio_map.mark_dirty();
        }

        if self.radio_map.is_dirty() {
            self.radio_map
                .rebuild(&self.channel, &self.cells, &self.ues);
        }

        self.result_buf.clear();
        for i in 0..self.cells.len() {
            self.step_cell(CellSlot::from_index(i));
        }

        self.clock.tick();
    }

    fn step_cell(&mut self, cell_slot: CellSlot) {
        self.grant_buf.clear();
        self.cells
            .mac_mut(cell_slot)
            .step(&self.slot_ctx, &mut self.grant_buf);

        let mut used: u16 = 0;
        for grant in &self.grant_buf {
            used = used.saturating_add(grant.prbs.count);
        }
        self.used_prbs[cell_slot.index()] = used;

        let result_start = self.result_buf.len();
        for gi in 0..self.grant_buf.len() {
            let grant = self.grant_buf[gi];
            let Some(&ue_slot) = self.ue_index.get(&grant.ue) else {
                continue;
            };
            let sinr = self.build_sinr(cell_slot, ue_slot, grant.ue);
            let result = self
                .phy
                .evaluate(&self.slot_ctx, &grant, &sinr, &mut self.rng);
            self.result_buf.push(result);
        }

        for ri in result_start..self.result_buf.len() {
            let result = self.result_buf[ri];
            self.cells
                .mac_mut(cell_slot)
                .on_result(&self.slot_ctx, &result);
        }
    }

    fn build_sinr(&self, serving: CellSlot, ue_slot: UeSlot, ue: UeId) -> SinrContext {
        let s = self.radio_map.rx(serving.index(), ue_slot.index());
        let total = self.radio_map.total(ue_slot.index());
        SinrContext {
            ue,
            serving: Watt::new(s),
            interference: Watt::new((total - s).max(0.0)),
            noise: self.thermal_noise,
        }
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

fn thermal_noise_for(ctx: &SlotContext) -> Watt {
    const BOLTZMANN: f64 = 1.380_649e-23;
    const TEMPERATURE_K: f64 = 290.0;
    Watt::new(BOLTZMANN * TEMPERATURE_K * ctx.bandwidth.value())
}