use nr_core::{Bits, Db, SimRng};
use nr_spec::McsTable;
use rand::RngExt;
use sap::{Grant, Phy, SinrContext, SlotContext, TransportResult};

pub struct SysPhy {
    mcs_table: McsTable,
    n_re_per_rb: u32,
}

impl SysPhy {
    pub fn new(mcs_table: McsTable, n_re_per_rb: u32) -> Self {
        Self {
            mcs_table,
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
        rng: &mut SimRng,
    ) -> TransportResult {
        let eff_sinr = sinr.sinr_db();

        let tb_size = self
            .mcs_table
            .tbs(
                grant.mcs_index,
                grant.prbs.count as u32,
                self.n_re_per_rb,
                1,
            )
            .unwrap_or(Bits::ZERO);

        let bler = block_error_rate(eff_sinr, grant.mcs_index);
        let u: f64 = rng.inner().random();
        let success = u >= bler;

        TransportResult {
            ue: grant.ue,
            tb_size,
            success,
            effective_sinr: eff_sinr,
        }
    }
}

fn required_sinr_db(mcs_index: u8) -> f64 {
    let lo = -5.0;
    let hi = 22.0;
    let max_idx = 28.0;
    let idx = (mcs_index as f64).min(max_idx);
    lo + (hi - lo) * (idx / max_idx)
}

fn block_error_rate(effective_sinr: Db, mcs_index: u8) -> f64 {
    let required = required_sinr_db(mcs_index);
    let delta = effective_sinr.value() - required;
    let steepness = 1.5;
    1.0 / (1.0 + (steepness * delta).exp())
}

#[allow(dead_code)]
fn residual_bler(effective_sinr: Db, mcs_index: u8, n_retx: u8) -> f64 {
    let single = block_error_rate(effective_sinr, mcs_index);
    single.powi(n_retx as i32 + 1)
}
