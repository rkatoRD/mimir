//! トラフィックモデル（policy 層）。
//!
//! - [`ConstantTraffic`]: 毎スロット固定ビットの決定論到着（健全性確認・デバッグ用）。
//! - [`OuPoissonTraffic`]: 設計 §15.1 の OU 変調 Poisson 到着（Cox 過程）。
//!   評価トラフィックの一級市民。平均到着率 λᵤ(t) が Ornstein–Uhlenbeck 過程に
//!   従って時間変化し、各スロットの到着数はそのときの λ を強度とする Poisson に従う。

use nr_core::{BearerId, Bits, Second, SimRng, UeId};
use rand::RngExt;
use rand_distr::{Distribution, StandardNormal};
use sap::{SlotContext, TrafficArrival, TrafficModel};

/// 毎スロット固定ビットを全 UE に投入する決定論トラフィック。
/// 待ち行列はほぼ生じないため遅延 KPI の評価には向かない（健全性確認用）。
#[allow(dead_code)]
pub struct ConstantTraffic {
    bits_per_slot: u64,
}

#[allow(dead_code)]
impl ConstantTraffic {
    pub fn new(bits_per_slot: u64) -> Self {
        Self { bits_per_slot }
    }
}

impl TrafficModel for ConstantTraffic {
    fn generate(
        &mut self,
        _ctx: &SlotContext,
        ues: &[UeId],
        out: &mut Vec<TrafficArrival>,
        _rng: &mut SimRng,
    ) {
        for &ue in ues {
            out.push(TrafficArrival {
                ue,
                bearer: BearerId::new(0),
                size: Bits::new(self.bits_per_slot),
            });
        }
    }
}

/// 非負性の扱い（設計 §15.1）。
/// - `Clamp`: `λ ← max(λ, 0)`。既定。σ ≪ μ ならバイアス無視可能。
/// - `LogOu`: 状態 X が OU、λ = exp(X)。常に正、定常分布は対数正規。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Positivity {
    Clamp,
    #[allow(dead_code)] // 強変動実験用（log-OU）。設定で選択可能にする予定
    LogOu,
}

/// OU 変調 Poisson 到着（Cox 過程）の構築パラメータ。
///
/// `dλ = θ(μ − λ)dt + σ dW`（Clamp）/ `dX = θ(ln μ − X)dt + σ dW, λ = exp(X)`（LogOu）。
#[derive(Debug, Clone, Copy)]
pub struct OuParams {
    /// 回帰速度 θ [1/s]。大きいほど μ へ速く戻る。
    pub theta: f64,
    /// 長期平均到着率 μ [packets/s]。
    pub mu: f64,
    /// 変動強度 σ [packets/s/√s]（Clamp）/ [1/√s]（LogOu）。
    pub sigma: f64,
    /// 固定パケット長。
    pub packet_size: Bits,
    /// λ 更新の間引き（スロット数、既定 1）。
    pub update_period: u32,
    /// 非負性の扱い。
    pub positivity: Positivity,
}

impl Default for OuParams {
    fn default() -> Self {
        Self {
            theta: 5.0,
            mu: 800.0,
            sigma: 600.0,
            packet_size: Bits::new(1024),
            update_period: 1,
            positivity: Positivity::Clamp,
        }
    }
}

pub struct OuPoissonTraffic {
    // 構築時定数（事前計算済み）。
    decay: f64,           // e^{-θΔ}
    diff_std: f64,        // σ√((1 - e^{-2θΔ}) / (2θ))
    mu: f64,              // Clamp: μ [pkt/s] / LogOu: ln μ
    mean_pkts_scale: f64, // Δslot [s]（λ[pkt/s] → スロット平均パケット数）
    packet_size: Bits,
    update_period: u32,
    positivity: Positivity,
    // 状態（SoA 密配列、消費順序の決定論とキャッシュ効率のため HashMap を使わない）。
    lambda: Vec<f64>, // Clamp: λ / LogOu: 状態 X
}

impl OuPoissonTraffic {
    /// `slot_duration`: 1 スロットの実時間（`Numerology::slot_duration()`）。
    /// `n_ues`: λ 状態配列の初期長（spawn/despawn で同期する場合は scenario が管理）。
    pub fn new(params: OuParams, slot_duration: Second, n_ues: usize) -> Self {
        let dt = slot_duration.value() * params.update_period as f64;
        let decay = (-params.theta * dt).exp();
        let variance = params.sigma * params.sigma * (1.0 - decay * decay) / (2.0 * params.theta);
        let diff_std = variance.max(0.0).sqrt();

        let (mu, init) = match params.positivity {
            Positivity::Clamp => (params.mu, params.mu),
            // 定常平均 ln μ を中心に X を初期化（λ = exp(X) の初期 ≈ μ）。
            Positivity::LogOu => {
                let ln_mu = params.mu.max(f64::MIN_POSITIVE).ln();
                (ln_mu, ln_mu)
            }
        };

        Self {
            decay,
            diff_std,
            mu,
            mean_pkts_scale: slot_duration.value(),
            packet_size: params.packet_size,
            update_period: params.update_period.max(1),
            positivity: params.positivity,
            lambda: vec![init; n_ues],
        }
    }

    /// UE 数が変わった場合に λ 状態配列を同期する（政策側が呼ぶ）。
    /// 伸長分は定常平均で初期化する。
    pub fn resize(&mut self, n_ues: usize) {
        let init = self.mu;
        self.lambda.resize(n_ues, init);
    }

    /// 現在の λ [pkt/s]（LogOu は exp(X) 換算）。
    #[inline]
    fn lambda_rate(&self, i: usize) -> f64 {
        match self.positivity {
            Positivity::Clamp => self.lambda[i],
            Positivity::LogOu => self.lambda[i].exp(),
        }
    }
}

impl TrafficModel for OuPoissonTraffic {
    fn generate(
        &mut self,
        ctx: &SlotContext,
        ues: &[UeId],
        out: &mut Vec<TrafficArrival>,
        rng: &mut SimRng,
    ) {
        if self.lambda.len() < ues.len() {
            self.resize(ues.len());
        }
        let do_update = ctx.elapsed.value() % self.update_period as u64 == 0;

        for (i, &ue) in ues.iter().enumerate() {
            if do_update {
                let z: f64 = StandardNormal.sample(rng.inner());
                // OU 厳密解離散化（Euler–Maruyama は更新周期依存のため禁止）。
                let x = self.mu + (self.lambda[i] - self.mu) * self.decay + self.diff_std * z;
                self.lambda[i] = match self.positivity {
                    Positivity::Clamp => x.max(0.0),
                    Positivity::LogOu => x, // λ = exp(X) は常に正
                };
            }

            let mean = self.lambda_rate(i) * self.mean_pkts_scale;
            let n = poisson_knuth(mean, rng);
            for _ in 0..n {
                out.push(TrafficArrival {
                    ue,
                    bearer: BearerId::new(0),
                    size: self.packet_size,
                });
            }
        }
    }
}

/// Knuth 法による Poisson サンプリング。λΔ ≪ 10 を想定（O(λΔ)、軽量）。
#[inline]
fn poisson_knuth(mean: f64, rng: &mut SimRng) -> u32 {
    if mean <= 0.0 {
        return 0;
    }
    let l = (-mean).exp();
    let mut k: u32 = 0;
    let mut p: f64 = 1.0;
    loop {
        let u: f64 = rng.inner().random();
        p *= u;
        if p <= l {
            return k;
        }
        k += 1;
        // 異常な大 mean に対するフェイルセーフ（決定論を壊さない固定上限）。
        if k > 1_000_000 {
            return k;
        }
    }
}
