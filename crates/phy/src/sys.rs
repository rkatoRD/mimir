use std::sync::Arc;

use l2s::L2sTables;
use nr_core::{Bits, Db, SimRng};
use nr_spec::McsTable;
use rand::RngExt;
use sap::{Grant, Phy, SinrContext, SlotContext, TransportResult};

/// しきい値中心ロジスティックの急峻さ [1/dB]。フォールバック式と共通。
const BLER_STEEPNESS: f64 = 1.5;

pub struct SysPhy {
    mcs_table: McsTable,
    n_re_per_rb: u32,
    /// CSV 由来 L2S テーブル（設計 §15.3 消費点 2）。Some なら BLER は
    /// しきい値中心曲線で評価し、ILLA（消費点 1）と同一データ上で整合させる。
    /// None またはしきい値未登録 MCS ではロジスティック近似へフォールバック。
    l2s: Option<Arc<L2sTables>>,
}

impl SysPhy {
    pub fn new(mcs_table: McsTable, n_re_per_rb: u32) -> Self {
        Self {
            mcs_table,
            n_re_per_rb,
            l2s: None,
        }
    }

    /// L2S テーブル付きで構築（消費点 2 を有効化）。
    pub fn with_l2s(mcs_table: McsTable, n_re_per_rb: u32, l2s: Option<Arc<L2sTables>>) -> Self {
        Self {
            mcs_table,
            n_re_per_rb,
            l2s,
        }
    }

    #[inline]
    fn bler(&self, eff_sinr: Db, mcs_index: u8) -> f64 {
        match &self.l2s {
            Some(tables) => tables
                .bler(eff_sinr, mcs_index, BLER_STEEPNESS)
                .unwrap_or_else(|| block_error_rate(eff_sinr, mcs_index)),
            None => block_error_rate(eff_sinr, mcs_index),
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
            .tbs(grant.mcs_index, grant.prbs.count as u32, self.n_re_per_rb, 1)
            .unwrap_or(Bits::ZERO);

        let bler = self.bler(eff_sinr, grant.mcs_index);
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

/// L2S テーブル不在時のフォールバック BLER（ロジスティック近似）。
fn block_error_rate(effective_sinr: Db, mcs_index: u8) -> f64 {
    let required = required_sinr_db(mcs_index);
    let delta = effective_sinr.value() - required;
    1.0 / (1.0 + (BLER_STEEPNESS * delta).exp())
}

#[allow(dead_code)]
fn residual_bler(effective_sinr: Db, mcs_index: u8, n_retx: u8) -> f64 {
    let single = block_error_rate(effective_sinr, mcs_index);
    single.powi(n_retx as i32 + 1)
}