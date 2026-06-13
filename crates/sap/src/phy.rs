use nr_core::SimRng;

use crate::messages::{Grant, SinrView, SlotContext, TransportResult};

pub trait Phy {
    /// 1 グラントのトランスポートブロック評価。
    ///
    /// SL: 実効 SINR → BLER → ベルヌーイ判定 + TBS 計算。
    /// LL: 波形生成 → チャネル畳み込み → 復調復号。
    ///
    /// SINR は [`SinrView`] で受け取る（設計 §4.4(c)）。`per_prb_linear` が
    /// `Some` のとき EESM 等で実効 SINR へ圧縮し、`None`（フェーズ1 互換）の
    /// ときは `wideband` をそのまま用いる。HARQ 再送（`grant.harq_attempt > 0`）は
    /// 再送合成による残留 BLER の改善を評価する（設計 §15.2）。
    fn evaluate(
        &mut self,
        ctx: &SlotContext,
        grant: &Grant,
        sinr: &SinrView,
        rng: &mut SimRng,
    ) -> TransportResult;
}
