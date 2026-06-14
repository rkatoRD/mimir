//! スケジューラ（policy 層、設計 §4.3「スケジューラは mac/common」/ §15.3 消費点 1）。
//!
//! [`Scheduler`] は「UE 選択 + PRB 配分 + MCS 選択(ILLA)」を担う。MAC は各 UE の
//! 状態を [`SchedulingRequest`] に集約してスケジューラへ渡し、スケジューラが
//! [`Grant`] を生成する。MCS は新規送信では `channel_quality`（実効 SINR
//! フィードバック）から L2S テーブルで選び、HARQ 再送では `forced_mcs` を
//! そのまま使う（チェイス合成は同一 MCS 前提）。
//!
//! 本モジュールは周波数再利用ポリシーの比較実験のため 2 実装を提供する:
//! - [`FullReuseScheduler`]: 全 PRB を全セルで再利用（reuse-1）。セル端で
//!   他セル干渉を受ける。協調なしベースライン。
//! - [`StaticSplitScheduler`]: セル固有の PRB 区間（直交分割）内で割り当てる。
//!   セル間で PRB が重ならないため干渉が構造的に消える（reuse-N の静的版）。
//!   協調手法の比較対象。
//!
//! どちらも 1 スロット 1 grant・ラウンドロビン UE 選択（既存 MAC 挙動を継承）。
//! PRB 配分の差（全幅 vs 固定窓）だけが両者の違いであり、それ以外の経路は共通。

use std::sync::Arc;

use l2s::L2sTables;
use nr_core::{Db, Direction};
use sap::{Grant, PrbAllocation, Scheduler, SchedulingRequest, SlotContext};

/// セルに割り当てられた PRB 窓（連続区間）。
#[derive(Debug, Clone, Copy)]
pub struct PrbWindow {
    pub start: u16,
    pub count: u16,
}

impl PrbWindow {
    pub fn new(start: u16, count: u16) -> Self {
        Self { start, count }
    }

    /// 全幅（reuse-1）。`total_prbs` 全部を 1 セルが使う。
    pub fn full(total_prbs: u16) -> Self {
        Self {
            start: 0,
            count: total_prbs,
        }
    }

    /// `n_cells` 等分のうち `cell_index` 番目の区間（直交分割）。
    /// 端数は先頭セルから 1 PRB ずつ配って取りこぼしを無くす。
    pub fn split(total_prbs: u16, n_cells: u16, cell_index: u16) -> Self {
        let base = total_prbs / n_cells;
        let rem = total_prbs % n_cells;
        // cell_index より前のセルが受け取った合計を start に積む。
        let start = base * cell_index + cell_index.min(rem);
        let count = base + if cell_index < rem { 1 } else { 0 };
        Self { start, count }
    }
}

/// MCS 選択ポリシー（ILLA）。L2S テーブルがあれば `channel_quality` から選び、
/// 無ければフォールバック固定 MCS（設計 §15.3 消費点 1 / フォールバック）。
struct McsSelector {
    l2s: Option<Arc<L2sTables>>,
    fallback_mcs: u8,
}

impl McsSelector {
    #[inline]
    fn select(&self, req: &SchedulingRequest) -> u8 {
        // 再送は確定 MCS 固定（チェイス合成、同一 MCS）。
        if req.harq_retx {
            return req.forced_mcs;
        }
        match (&self.l2s, req.channel_quality) {
            (Some(tables), Some(sinr)) => tables.select_mcs(sinr),
            _ => self.fallback_mcs,
        }
    }
}

/// 共通のラウンドロビン UE 選択 + grant 生成。`window` が PRB 配分を決める。
struct RoundRobinCore {
    window: PrbWindow,
    mcs: McsSelector,
    /// 次に優先する requests 内の開始位置（ラウンドロビンポインタ）。
    cursor: usize,
}

impl RoundRobinCore {
    fn schedule(&mut self, requests: &[SchedulingRequest], out: &mut Vec<Grant>) {
        let n = requests.len();
        if n == 0 || self.window.count == 0 {
            return;
        }
        // cursor から 1 周し、送信すべき（backlog>0 または再送）最初の UE を選ぶ。
        for k in 0..n {
            let idx = (self.cursor + k) % n;
            let req = &requests[idx];
            let needs_tx = req.harq_retx || req.backlog.value() > 0;
            if needs_tx {
                let mcs_index = self.mcs.select(req);
                out.push(Grant {
                    ue: req.ue,
                    prbs: PrbAllocation::new(self.window.start, self.window.count),
                    mcs_index,
                    direction: Direction::Downlink,
                    harq_process: 0,
                    harq_attempt: req.harq_attempt,
                });
                // 次スロットは次の UE から（公平性）。
                self.cursor = (idx + 1) % n;
                return;
            }
        }
    }
}

/// reuse-1: 全 PRB を使う。協調なしベースライン。
pub struct FullReuseScheduler {
    core: RoundRobinCore,
}

impl FullReuseScheduler {
    pub fn new(total_prbs: u16, fallback_mcs: u8, l2s: Option<Arc<L2sTables>>) -> Self {
        Self {
            core: RoundRobinCore {
                window: PrbWindow::full(total_prbs),
                mcs: McsSelector { l2s, fallback_mcs },
                cursor: 0,
            },
        }
    }
}

impl Scheduler for FullReuseScheduler {
    fn schedule(
        &mut self,
        _ctx: &SlotContext,
        requests: &[SchedulingRequest],
        out: &mut Vec<Grant>,
    ) {
        self.core.schedule(requests, out);
    }
}

/// 静的分割: セル固有 PRB 区間内で割り当てる（直交、干渉なし）。
pub struct StaticSplitScheduler {
    core: RoundRobinCore,
}

impl StaticSplitScheduler {
    /// `n_cells` 等分のうち `cell_index` 番目の区間を使う。
    pub fn new(
        total_prbs: u16,
        n_cells: u16,
        cell_index: u16,
        fallback_mcs: u8,
        l2s: Option<Arc<L2sTables>>,
    ) -> Self {
        Self {
            core: RoundRobinCore {
                window: PrbWindow::split(total_prbs, n_cells, cell_index),
                mcs: McsSelector { l2s, fallback_mcs },
                cursor: 0,
            },
        }
    }
}

impl Scheduler for StaticSplitScheduler {
    fn schedule(
        &mut self,
        _ctx: &SlotContext,
        requests: &[SchedulingRequest],
        out: &mut Vec<Grant>,
    ) {
        self.core.schedule(requests, out);
    }
}
