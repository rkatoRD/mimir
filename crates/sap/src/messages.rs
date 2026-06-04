use nr_core::{BearerId, Bits, CellId, Db, Direction, Hz, SfnSlot, UeId, Watt};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrbAllocation {
    pub start: u16,
    pub count: u16,
}

impl PrbAllocation {
    pub const fn new(start: u16, count: u16) -> Self {
        Self { start, count }
    }

    pub const fn len(self) -> u16 {
        self.count
    }

    pub const fn is_empty(self) -> bool {
        self.count == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Grant {
    pub ue: UeId,
    pub prbs: PrbAllocation,
    pub mcs_index: u8,
    pub direction: Direction,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChannelSample {
    pub from: CellId,
    pub to: UeId,
    pub rx_power: Watt,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SinrContext {
    pub ue: UeId,
    pub serving: Watt,
    pub interference: Watt,
    pub noise: Watt,
}

impl SinrContext {
    pub fn sinr_db(&self) -> Db {
        let linear = self.serving / (self.interference + self.noise);
        Db::new(10.0 * linear.log10())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransportResult {
    pub ue: UeId,
    pub tb_size: Bits,
    pub success: bool,
    pub effective_sinr: Db,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrafficArrival {
    pub ue: UeId,
    pub bearer: BearerId,
    pub size: Bits,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CoordinationMessage {
    PowerBudget {
        cell: CellId,
        prbs: PrbAllocation,
        max_power: Watt,
    },
    MutedPrbs {
        cell: CellId,
        prbs: PrbAllocation,
    },
    LoadReport {
        cell: CellId,
        active_ues: u16,
        used_prbs: u16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SlotContext {
    pub slot: SfnSlot,
    pub bandwidth: Hz,
    pub total_prbs: u16,
}
