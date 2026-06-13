//! モビリティモデル（policy 層、設計 §4.3「共通」/ §ロードマップ フェーズ2）。
//!
//! [`MobilityModel`] は out-parameter 形式（`out: &mut [Point]`）で、
//! 各 UE の次スロット位置を書き込む。乱数は engine 所有の [`SimRng`] を
//! `step` 引数で受け取り、自前 RNG を持たない（設計 §6.1 / §8.1）。
//!
//! - [`StaticMobility`]: 位置不変（固定 UE シナリオ。`step` は current をコピー）。
//! - [`LinearMobility`]: UE ごとに固定速度ベクトルで等速直進。境界で反射。
//! - [`RandomWalkMobility`]: スロット毎に方向をランダム化する 2D ランダムウォーク。
//!
//! 速度はメートル毎秒で与え、構築時に 1 スロットあたりの変位 [m] へ畳み込む
//! （ホットパスでの時間換算を排除）。
//!
//! 現フェーズの runner 既定シナリオは固定 UE のため、これらは engine の
//! `step_mobility` 接続点に渡して移動 UE シナリオを組む将来用 API として提供する。
#![allow(dead_code)]

use nr_core::{Meter, Point, Second, SimRng, UeId};
use rand::RngExt;
use sap::{MobilityModel, SlotContext};

/// 矩形境界 [m]（2D）。UE はこの内側に留まる。
#[derive(Debug, Clone, Copy)]
pub struct Bounds {
    pub min_x: f64,
    pub max_x: f64,
    pub min_y: f64,
    pub max_y: f64,
}

impl Bounds {
    pub fn new(min_x: f64, max_x: f64, min_y: f64, max_y: f64) -> Self {
        Self {
            min_x,
            max_x,
            min_y,
            max_y,
        }
    }
}

/// 位置不変。固定 UE シナリオの既定。`step` は current をそのまま out へ写す。
pub struct StaticMobility;

impl MobilityModel for StaticMobility {
    fn step(
        &mut self,
        _ctx: &SlotContext,
        _ues: &[UeId],
        current: &[Point],
        out: &mut [Point],
        _rng: &mut SimRng,
    ) {
        out[..current.len()].copy_from_slice(current);
    }
}

/// UE ごとの固定速度ベクトルで等速直進。矩形境界で速度成分を反転（反射）。
pub struct LinearMobility {
    /// 1 スロットあたりの変位 [m]（vx, vy）。SoA 密配列、UE 順。
    step_xy: Vec<(f64, f64)>,
    bounds: Bounds,
}

impl LinearMobility {
    /// `velocities`: UE ごとの (vx, vy) [m/s]。`slot_duration`: 1 スロット実時間。
    pub fn new(velocities: &[(f64, f64)], slot_duration: Second, bounds: Bounds) -> Self {
        let dt = slot_duration.value();
        let step_xy = velocities
            .iter()
            .map(|&(vx, vy)| (vx * dt, vy * dt))
            .collect();
        Self { step_xy, bounds }
    }

    /// UE 数が変わった場合の同期（伸長分は静止）。
    pub fn resize(&mut self, n_ues: usize) {
        self.step_xy.resize(n_ues, (0.0, 0.0));
    }
}

impl MobilityModel for LinearMobility {
    fn step(
        &mut self,
        _ctx: &SlotContext,
        ues: &[UeId],
        current: &[Point],
        out: &mut [Point],
        _rng: &mut SimRng,
    ) {
        if self.step_xy.len() < ues.len() {
            self.resize(ues.len());
        }
        for i in 0..current.len() {
            let p = current[i];
            let (mut dx, mut dy) = self.step_xy[i];
            let mut nx = p.x.value() + dx;
            let mut ny = p.y.value() + dy;

            // 境界反射: 範囲外なら折り返し、速度成分も反転して次スロットへ持続。
            if nx < self.bounds.min_x {
                nx = 2.0 * self.bounds.min_x - nx;
                dx = -dx;
            } else if nx > self.bounds.max_x {
                nx = 2.0 * self.bounds.max_x - nx;
                dx = -dx;
            }
            if ny < self.bounds.min_y {
                ny = 2.0 * self.bounds.min_y - ny;
                dy = -dy;
            } else if ny > self.bounds.max_y {
                ny = 2.0 * self.bounds.max_y - ny;
                dy = -dy;
            }
            self.step_xy[i] = (dx, dy);
            out[i] = Point::new(Meter::new(nx), Meter::new(ny), p.z);
        }
    }
}

/// 2D ランダムウォーク。各スロットで一様方向 × 固定速度で移動、境界でクランプ。
pub struct RandomWalkMobility {
    /// 1 スロットあたりの移動距離 [m]（UE 共通の速さ）。
    speed_per_slot: f64,
    bounds: Bounds,
}

impl RandomWalkMobility {
    /// `speed_mps`: 速さ [m/s]。`slot_duration`: 1 スロット実時間。
    pub fn new(speed_mps: f64, slot_duration: Second, bounds: Bounds) -> Self {
        Self {
            speed_per_slot: speed_mps * slot_duration.value(),
            bounds,
        }
    }
}

impl MobilityModel for RandomWalkMobility {
    fn step(
        &mut self,
        _ctx: &SlotContext,
        _ues: &[UeId],
        current: &[Point],
        out: &mut [Point],
        rng: &mut SimRng,
    ) {
        // UE 順に乱数を消費（消費順序固定 = 決定論、設計 §8.1）。
        for i in 0..current.len() {
            let p = current[i];
            let theta: f64 = rng.inner().random::<f64>() * std::f64::consts::TAU;
            let nx = (p.x.value() + self.speed_per_slot * theta.cos())
                .clamp(self.bounds.min_x, self.bounds.max_x);
            let ny = (p.y.value() + self.speed_per_slot * theta.sin())
                .clamp(self.bounds.min_y, self.bounds.max_y);
            out[i] = Point::new(Meter::new(nx), Meter::new(ny), p.z);
        }
    }
}
