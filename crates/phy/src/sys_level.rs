pub mod bler_lookup;
pub mod effective_sinr;
pub mod retx_bler;
pub mod throughput;

use nr_core::Db;
use nr_spec::mcs::McsTable;
use sap::{Grant, Phy, SinrContext, SlotContext, TransportResult};

use crate::common::mcs_util::McsConfig;

pub struct SysPhy {
    mcs: McsConfig,
    n_re_per_rb: u32,
}

impl SysPhy {
    pub fn new(mcs_table: McsTable, n_re_per_rb: u32) -> Self {
        Self {
            mcs: McsConfig::new(mcs_table),
            n_re_per_rb,
        }
    }
}

impl Default for SysPhy {
    fn default() -> Self {
        Self::new(McsTable::Table1, 120)
    }
}

impl Phy for SysPhy {
    fn evaluate(
        &mut self,
        _ctx: &SlotContext,
        grant: &Grant,
        sinr: &SinrContext,
    ) -> TransportResult {
        let eff_sinr = effective_sinr::from_context(sinr);
        let tb_size = throughput::transport_block_size(
            &self.mcs,
            grant.mcs_index,
            grant.prbs.count as u32,
            self.n_re_per_rb,
        );
        let bler = bler_lookup::block_error_rate(eff_sinr, grant.mcs_index);
        let success = bler < 0.5;

        TransportResult {
            ue: grant.ue,
            tb_size,
            success,
            effective_sinr: eff_sinr,
        }
    }

    fn block_error_rate(&self, effective_sinr: Db, mcs_index: u8) -> f64 {
        bler_lookup::block_error_rate(effective_sinr, mcs_index)
    }
}
