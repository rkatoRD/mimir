use std::sync::Arc;

use l2s::L2sTables;
use nr_core::{Bits, Db, SimRng};
use nr_spec::McsTable;
use rand::RngExt;
use sap::{Grant, Phy, SinrView, SlotContext, TransportResult};

/// しきい値中心ロジスティックの急峻さ [1/dB]。フォールバック式と共通。
const BLER_STEEPNESS: f64 = 1.5;

/// HARQ チェイス合成の 1 再送あたり実効 SINR 利得 [dB]（システムレベル近似）。
/// 再送ごとに受信エネルギーが累積し実効 SINR が上がる効果を、試行回数に
/// 比例した dB 加算で近似する（設計 §15.2、本来は level-link で較正）。
const HARQ_COMBINING_GAIN_DB: f64 = 3.0;

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

    /// グラント MCS の変調次数 Qm を引く（EESM β 選択に使う）。未登録は QPSK 扱い。
    #[inline]
    fn modulation_order(&self, mcs_index: u8) -> u8 {
        self.mcs_table
            .entry(mcs_index)
            .map(|e| e.modulation_order)
            .unwrap_or(2)
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

    /// [`SinrView`] から評価に用いる実効 SINR [dB] を決める。
    ///
    /// - `per_prb_linear == None`（フェーズ1 互換）: ワイドバンド SINR をそのまま。
    /// - `Some(γ)`: EESM で per-PRB 線形 SINR 列を実効 SINR へ圧縮（§4.2 フェーズ2）。
    ///   β は変調次数依存の代表値（将来 level-link で較正、§4.2）。
    #[inline]
    fn effective_sinr_db(&self, sinr: &SinrView, mcs_index: u8) -> Db {
        match sinr.per_prb_linear {
            None => sinr.wideband.sinr_db(),
            Some(per_prb) if per_prb.is_empty() => sinr.wideband.sinr_db(),
            Some(per_prb) => {
                let beta = eesm_beta(self.modulation_order(mcs_index));
                eesm_effective_sinr_db(per_prb, beta)
            }
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
        sinr: &SinrView,
        rng: &mut SimRng,
    ) -> TransportResult {
        let base_sinr = self.effective_sinr_db(sinr, grant.mcs_index);

        // HARQ 再送合成: 試行回数に比例した実効 SINR 利得を加算（チェイス合成近似）。
        // harq_attempt = 0（初送）なら利得ゼロ。effective_sinr へ反映し、
        // フィードバック（TransportResult.effective_sinr）も合成後の値を報告する。
        let eff_sinr = if grant.harq_attempt > 0 {
            Db::new(base_sinr.value() + grant.harq_attempt as f64 * HARQ_COMBINING_GAIN_DB)
        } else {
            base_sinr
        };

        let tb_size = self
            .mcs_table
            .tbs(
                grant.mcs_index,
                grant.prbs.count as u32,
                self.n_re_per_rb,
                1,
            )
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

/// EESM 較正パラメータ β [線形]（変調次数別の代表値）。
///
/// EESM: `SINR_eff = -β·ln( (1/N)·Σ exp(-γ_n/β) )`。β は変調方式ごとに
/// リンクレベルシミュレーションで較正される量で、ここでは 3GPP で広く使われる
/// 代表値を用いる。本来は `level-link`（設計 §4.1/§4.2）が生成する。
#[inline]
fn eesm_beta(modulation_order: u8) -> f64 {
    match modulation_order {
        2 => 1.49, // QPSK
        4 => 4.56, // 16QAM
        6 => 16.5, // 64QAM
        8 => 56.0, // 256QAM（代表値）
        _ => 1.49,
    }
}

/// per-PRB 線形 SINR 列を EESM で単一の実効 SINR [dB] へ圧縮する。
///
/// `γ_n` は線形 SINR。`SINR_eff_linear = -β·ln( mean( exp(-γ_n/β) ) )`。
/// 総和は逐次加算（消費順序固定 = 決定論、§8.1）。
#[inline]
fn eesm_effective_sinr_db(per_prb_linear: &[f64], beta: f64) -> Db {
    let n = per_prb_linear.len() as f64;
    let mut acc = 0.0;
    for &g in per_prb_linear {
        acc += (-g / beta).exp();
    }
    let mean = acc / n;
    // mean ∈ (0,1]; ln(mean) ≤ 0 なので eff_linear ≥ 0。
    let eff_linear = (-beta * mean.ln()).max(f64::MIN_POSITIVE);
    Db::new(10.0 * eff_linear.log10())
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
