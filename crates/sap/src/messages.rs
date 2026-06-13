use nr_core::{BearerId, Bits, CellId, Db, Direction, Hz, SfnSlot, Slot, UeId, Watt};

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
    /// HARQ プロセス ID（NR のストップ&ウェイト並列プロセス識別）。
    /// 初送・再送を同一プロセスで束ねる。単一プロセス構成では 0 固定。
    pub harq_process: u8,
    /// HARQ 送信試行回数。0 = 初送、1.. = n 回目の再送。
    /// PHY は再送合成（チェイス合成等）による残留 BLER の改善をこの値で評価する
    /// （設計 §15.2 / phy/sys の `residual_bler`）。
    pub harq_attempt: u8,
}

impl Grant {
    /// HARQ なし（初送固定）の Grant を作るヘルパ。既存呼び出し側の移行を容易にする。
    #[inline]
    pub const fn new(ue: UeId, prbs: PrbAllocation, mcs_index: u8, direction: Direction) -> Self {
        Self {
            ue,
            prbs,
            mcs_index,
            direction,
            harq_process: 0,
            harq_attempt: 0,
        }
    }
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
    /// ワイドバンド実効 SINR（線形）。`serving / (interference + noise)`。
    #[inline]
    pub fn linear(&self) -> f64 {
        self.serving / (self.interference + self.noise)
    }

    pub fn sinr_db(&self) -> Db {
        Db::new(10.0 * self.linear().log10())
    }
}

/// SINR の借用ビュー（設計 §4.4(c)）。EESM/ハイブリッド用の per-PRB 拡張点。
///
/// `wideband` は常に有効なワイドバンド単一 SINR（フェーズ1 互換）。
/// `per_prb_linear` は engine 所有の再利用バッファへの借用（線形 SINR の列）で、
/// `Some` のとき PHY は EESM 等で実効 SINR へ圧縮する。`None`（既定）のとき
/// PHY は `wideband` のみを読む。**per-PRB 配列を所有させない（借用にする）**
/// ことがゼロアロケーションの要点（ホットパス確保なし）。
#[derive(Debug, Clone, Copy)]
pub struct SinrView<'a> {
    pub wideband: SinrContext,
    pub per_prb_linear: Option<&'a [f64]>,
}

impl<'a> SinrView<'a> {
    /// ワイドバンドのみ（per-PRB なし）のビュー。フェーズ1 互換の既定経路。
    #[inline]
    pub fn wideband(ctx: SinrContext) -> Self {
        Self {
            wideband: ctx,
            per_prb_linear: None,
        }
    }

    /// per-PRB 線形 SINR 列を伴うビュー（EESM 経路）。
    #[inline]
    pub fn with_per_prb(ctx: SinrContext, per_prb_linear: &'a [f64]) -> Self {
        Self {
            wideband: ctx,
            per_prb_linear: Some(per_prb_linear),
        }
    }

    /// このビューの UE。
    #[inline]
    pub fn ue(&self) -> UeId {
        self.wideband.ue
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
    pub sfn_slot: SfnSlot,
    pub elapsed: Slot,
    pub bandwidth: Hz,
    pub total_prbs: u16,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PacketCompletion {
    pub ue: UeId,
    pub bearer: BearerId,
    pub size: Bits,
    pub arrival: Slot,
    pub completion: Slot,
}
