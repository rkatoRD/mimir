//! 3GPP TR 38.901 Indoor Factory (InF) パスロスモデル（設計 §ロードマップ フェーズ2）。
//!
//! Table 7.4.1-1 の InF を実装する。InF は clutter density / height で
//! サブシナリオが分かれる:
//!
//! | サブシナリオ | clutter | BS 高 |
//! |---|---|---|
//! | InF-SL | sparse | 低 |
//! | InF-DL | dense  | 低 |
//! | InF-SH | sparse | 高 |
//! | InF-DH | dense  | 高 |
//!
//! パスロス式（fc は GHz、d は m）:
//! - LOS:  `PL = 31.84 + 21.50·log10(d3D) + 19.00·log10(fc)`,  σ_SF = 4.0 dB
//! - NLOS: サブシナリオ別の式と LOS の **最大値**（38.901 規定）:
//!   - SL: `33.0  + 25.5·log10(d3D) + 20·log10(fc)`, σ_SF = 5.7
//!   - DL: `18.6  + 35.7·log10(d3D) + 20·log10(fc)`, σ_SF = 7.2
//!   - SH: `32.4  + 23.0·log10(d3D) + 20·log10(fc)`, σ_SF = 5.9
//!   - DH: `33.63 + 21.9·log10(d3D) + 20·log10(fc)`, σ_SF = 4.0
//!
//! LOS 確率は完全な clutter ジオメトリ依存だが、本フェーズは距離依存の
//! 指数モデル `P_LOS = exp(-d2D / k_subsc)` を用いる。LOS/NLOS 判定は
//! per-link 決定論ハッシュ（RNG 列非消費、同一リンクは常に同一）で行う。
//!
//! シャドウイングは大規模確率実現量だが、本実装は **per-link 決定論ハッシュ**で
//! 引く（リンク (cell, ue) と固定シードから直接 N(0,σ²) を導出）。engine の
//! RNG 列を消費しないため、RadioMap の dirty 再計算をまたいでも同一リンクは
//! 常に同一値になり、呼び出し順非依存で決定論（設計 §8.1）を満たす。σ は
//! LOS/NLOS で異なる値を per-link で適用する。静的 UE シナリオでは初回 `update`
//! のみ「変化あり」（`true`）を返して RadioMap を一度だけ確定させる。

use nr_core::{CellId, Db, Hz, Point, SimRng, UeId, Watt};
use sap::{ChannelModel, SlotContext};

use crate::shadowing::ShadowField;

const MIN_DISTANCE_M: f64 = 1.0;

/// LOS の σ_SF は全 InF サブシナリオ共通で 4.0 dB（38.901）。
const LOS_SIGMA_SF: f64 = 4.0;

/// InF サブシナリオ（38.901 Table 7.2-4 / 7.4.1-1）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfSubScenario {
    /// Sparse clutter, Low BS height.
    SparseLow,
    /// Dense clutter, Low BS height.
    DenseLow,
    /// Sparse clutter, High BS height.
    SparseHigh,
    /// Dense clutter, High BS height.
    DenseHigh,
}

impl InfSubScenario {
    /// NLOS 平均パスロス係数 (offset, dist_coeff, freq_coeff) と σ_SF [dB]。
    #[inline]
    fn nlos_params(self) -> (f64, f64, f64, f64) {
        match self {
            InfSubScenario::SparseLow => (33.0, 25.5, 20.0, 5.7),
            InfSubScenario::DenseLow => (18.6, 35.7, 20.0, 7.2),
            InfSubScenario::SparseHigh => (32.4, 23.0, 20.0, 5.9),
            InfSubScenario::DenseHigh => (33.63, 21.9, 20.0, 4.0),
        }
    }

    /// LOS 確率の距離スケール k [m]: `P_LOS = exp(-d2D / k)`。
    #[inline]
    fn los_scale_m(self) -> f64 {
        match self {
            InfSubScenario::SparseLow | InfSubScenario::SparseHigh => 54.55,
            InfSubScenario::DenseLow | InfSubScenario::DenseHigh => 13.0,
        }
    }
}

pub struct InfChannel {
    fc_ghz: f64,
    subsc: InfSubScenario,
    /// LOS/NLOS 判定用の固定シード（per-link 決定論ハッシュ）。
    los_seed: u64,
    /// シャドウイング場（σ=0 で無効）。engine 所有 RNG を update で消費して生成。
    shadow: ShadowField,
}

impl InfChannel {
    /// `enable_shadowing=false` でシャドウイング無効（平均 PL のみ）。
    ///
    /// シャドウイング σ は per-link の LOS/NLOS で異なるが、`ShadowField` は
    /// 単一 σ の標準正規場を持つ。ここでは代表 σ として **NLOS 値**を渡し、
    /// LOS リンクには `rx_power_batch` 内で `LOS_SIGMA_SF / σ_nlos` 比で
    /// スケールし直して per-link σ を厳密に反映する。
    pub fn new(fc: Hz, subsc: InfSubScenario, los_seed: u64, enable_shadowing: bool) -> Self {
        let (_, _, _, sigma_nlos) = subsc.nlos_params();
        let sigma = if enable_shadowing { sigma_nlos } else { 0.0 };
        Self {
            fc_ghz: fc.to_ghz(),
            subsc,
            los_seed,
            shadow: ShadowField::new(sigma),
        }
    }

    pub fn with_defaults(fc: Hz, subsc: InfSubScenario) -> Self {
        Self::new(fc, subsc, 0x5341_5F49_4E46, true)
    }

    /// このリンク（セル→UE）が LOS か。距離依存確率 + 決定論ハッシュ。
    #[inline]
    fn is_los(&self, from: CellId, to: UeId, d2d: f64) -> bool {
        let p_los = (-d2d / self.subsc.los_scale_m()).exp();
        link_uniform(self.los_seed, from, to) < p_los
    }

    #[inline]
    fn mean_pathloss_db(&self, d3d: f64, is_los: bool) -> f64 {
        let log_d = d3d.max(MIN_DISTANCE_M).log10();
        let log_f = self.fc_ghz.log10();
        let los_pl = 31.84 + 21.50 * log_d + 19.00 * log_f;
        if is_los {
            los_pl
        } else {
            let (off, dc, fc_c, _) = self.subsc.nlos_params();
            (off + dc * log_d + fc_c * log_f).max(los_pl)
        }
    }

    /// 平均 PL + per-link σ スケール済みシャドウイングを織り込んだ受信電力。
    /// `shadow_db` は `ShadowField` から引いた「σ_nlos スケールの正規実現」。
    #[inline]
    fn rx_power_with_shadow(
        &self,
        from: CellId,
        to: UeId,
        tx_power: Watt,
        tx_pos: Point,
        rx_pos: Point,
        shadow_db_nlos_scale: f64,
    ) -> Watt {
        let d3d = tx_pos.distance_3d(&rx_pos).value();
        let d2d = tx_pos.distance_2d(&rx_pos).value();
        let is_los = self.is_los(from, to, d2d);
        let mut pl_db = self.mean_pathloss_db(d3d, is_los);

        if self.shadow.is_enabled() {
            // 場は σ_nlos スケール。LOS リンクは σ_LOS へ比率変換。
            let (_, _, _, sigma_nlos) = self.subsc.nlos_params();
            let applied = if is_los {
                shadow_db_nlos_scale * (LOS_SIGMA_SF / sigma_nlos)
            } else {
                shadow_db_nlos_scale
            };
            pl_db += applied;
        }

        let rx_dbm = tx_power.to_dbm() - Db::new(pl_db);
        rx_dbm.to_watt()
    }

    #[inline]
    pub fn los_sigma_sf(&self) -> f64 {
        LOS_SIGMA_SF
    }
}

impl ChannelModel for InfChannel {
    fn update(&mut self, _ctx: &SlotContext, _rng: &mut SimRng) -> bool {
        // シャドウイングは per-link 決定論ハッシュ（rx_power 内）で引くため、
        // engine の RNG 列は消費しない。静的シナリオでは初回のみ「受信電力に
        // 影響する変化あり」を通知して RadioMap を一度だけ再計算させ、以後は
        // 変化なし（false）。移動 UE は engine 側の位置更新で dirty が立つ。
        if self.shadow.is_enabled() && !self.shadow.is_generated() {
            self.shadow.mark_generated();
            return true;
        }
        false
    }

    fn rx_power(
        &self,
        from: CellId,
        to: UeId,
        tx_power: Watt,
        tx_pos: Point,
        rx_pos: Point,
    ) -> Watt {
        // 単一ペア経路（低頻度・デバッグ用）。シャドウイングは per-link
        // 決定論ハッシュで引く（場の有無に依存せず単体で再現可能）。
        let shadow = if self.shadow.is_enabled() {
            let (_, _, _, sigma_nlos) = self.subsc.nlos_params();
            sigma_nlos * link_normal(self.los_seed ^ 0x5348_4144_4F57, from, to)
        } else {
            0.0
        };
        self.rx_power_with_shadow(from, to, tx_power, tx_pos, rx_pos, shadow)
    }

    fn rx_power_batch(
        &self,
        from: CellId,
        tx_power: Watt,
        tx_pos: Point,
        ues: &[UeId],
        rx_pos: &[Point],
        out: &mut [Watt],
    ) {
        // RadioMap の主経路。シャドウイングは per-link 決定論ハッシュで引くため、
        // バッチ内でも単一ペアと同一値になり、呼び出し順非依存（決定論）。
        let (_, _, _, sigma_nlos) = self.subsc.nlos_params();
        for i in 0..ues.len() {
            let to = ues[i];
            let shadow = if self.shadow.is_enabled() {
                sigma_nlos * link_normal(self.los_seed ^ 0x5348_4144_4F57, from, to)
            } else {
                0.0
            };
            out[i] = self.rx_power_with_shadow(from, to, tx_power, tx_pos, rx_pos[i], shadow);
        }
    }
}

/// リンク (cell, ue) と seed から決定論的に [0,1) 一様乱数を引く。
/// SplitMix64 風の混合ハッシュ。RNG 列を消費しないので呼び出し順に依存せず、
/// 同一リンクは常に同一値（再現性 = 設計 §8.1）。
#[inline]
fn link_uniform(seed: u64, from: CellId, to: UeId) -> f64 {
    let mut x = seed
        ^ (from.value() as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (to.value() as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    ((x >> 11) as f64) * (1.0 / 9_007_199_254_740_992.0)
}

/// リンク決定論ハッシュから標準正規 N(0,1)（Box–Muller）。
#[inline]
fn link_normal(seed: u64, from: CellId, to: UeId) -> f64 {
    let u1 = link_uniform(seed, from, to).max(f64::MIN_POSITIVE);
    let u2 = link_uniform(seed ^ 0xA5A5_5A5A_A5A5_5A5A, from, to);
    (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
}
