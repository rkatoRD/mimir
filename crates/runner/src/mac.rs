//! ラウンドロビン MAC（policy 層）+ HARQ 状態機械（設計 §15.2 / §ロードマップ フェーズ2）。
//!
//! パケットキュー MAC に HARQ ストップ&ウェイトプロセスを載せる。フェーズ1 の
//! 「楽観引き当て + 失敗時キュー差し戻し」を、フェーズ2 では **HARQ プロセスが
//! 送信中 TB を保持し、NACK で再送・ACK で確定**する正規の状態機械に置き換える。
//!
//! 設計上の要点:
//! - TB ↔ パケット集合の対応はグラント発行時に確定し、HARQ プロセスが `drained`
//!   （各パケットから引いた量）を保持する（§15.2 の落とし穴回避: 結果到着時に
//!   キュー先頭から数え直さない）。
//! - 失敗時はデータをキューへ戻さず HARQ プロセスに留め、`harq_attempt` を増やして
//!   再送待ちにする。PHY 側は `harq_attempt` で再送合成利得を評価する（方式 A）。
//! - 最大再送回数（`max_harq_attempts`）超過で TB を破棄し、含むパケットの
//!   引き当て分をキューへ戻す（再スケジュール対象に復帰 = パケットロスにしない）。
//! - 単一 HARQ プロセス構成（プロセス並列度 1）。NR の 8/16 プロセス並列は
//!   スループット上の効果が本シミュレーションのスロット同期 SL では限定的なため、
//!   フェーズ2 は再送機構の正しさを優先して 1 プロセスから入る。

use std::collections::VecDeque;
use std::sync::Arc;

use l2s::L2sTables;
use nr_core::{BearerId, Bits, Db, Direction, Slot, UeId};
use nr_spec::McsTable;
use sap::{
    Grant, Mac, PacketCompletion, PrbAllocation, SlotContext, TrafficArrival, TransportResult,
};

/// 既定の最大 HARQ 送信試行回数（初送 + 最大 3 再送 = 4、NR 慣行）。
const DEFAULT_MAX_HARQ_ATTEMPTS: u8 = 4;

#[derive(Debug, Clone, Copy)]
struct QueuedPacket {
    bearer: BearerId,
    size: Bits,
    remaining: Bits,
    arrival: Slot,
}

/// 送信中（in-flight）の HARQ プロセス。1 TB 分の引き当て情報を保持する。
struct HarqProcess {
    /// この TB で各先頭パケットから引いた量（キュー先頭からの並び順）。
    drained: Vec<Bits>,
    /// 確定済み MCS（再送でも同一 MCS = チェイス合成前提）。
    mcs_index: u8,
    /// 送信試行回数。0 = 初送、1.. = n 回目の再送。
    attempt: u8,
}

struct UeQueue {
    ue: UeId,
    packets: VecDeque<QueuedPacket>,
    backlog: Bits,
    last_sinr: Option<Db>,
    /// 送信中の HARQ プロセス（単一プロセス構成なので Option 1 個）。
    /// Some の間はこの UE は新規 TB を発行せず、再送のみを行う。
    harq: Option<HarqProcess>,
}

impl UeQueue {
    fn new(ue: UeId) -> Self {
        Self {
            ue,
            packets: VecDeque::new(),
            backlog: Bits::ZERO,
            last_sinr: None,
            harq: None,
        }
    }

    fn push(&mut self, bearer: BearerId, size: Bits, arrival: Slot) {
        self.packets.push_back(QueuedPacket {
            bearer,
            size,
            remaining: size,
            arrival,
        });
        self.backlog += size;
    }

    /// この UE が送信すべきものを持つか（新規 backlog または再送待ち HARQ）。
    #[inline]
    fn has_work(&self) -> bool {
        self.harq.is_some() || self.backlog.value() > 0
    }
}

pub struct RoundRobinMac {
    total_prbs: u16,
    fallback_mcs: u8,
    /// TB ビット予算算出用の数表（PHY と同一 → 引き当て量と配送量が一致）。
    mcs_table: McsTable,
    n_re_per_rb: u32,
    l2s: Option<Arc<L2sTables>>,
    max_harq_attempts: u8,
    order: VecDeque<usize>,
    queues: Vec<UeQueue>,
    completions: Vec<PacketCompletion>,
    /// このスロットで発行した TB の UE（on_result と突き合わせる）。
    /// 単一 grant/slot 構成だが、複数セル束ね等の将来拡張に備え Vec で持つ。
    in_flight_ue: Vec<UeId>,
    /// HARQ 再送で破棄された TB の数（KPI: パケット再投入回数の観測用）。
    dropped_tbs: u64,
}

impl RoundRobinMac {
    #[allow(dead_code)]
    pub fn new(
        total_prbs: u16,
        mcs_index: u8,
        mcs_table: McsTable,
        n_re_per_rb: u32,
        ues: &[UeId],
    ) -> Self {
        Self::with_l2s(total_prbs, mcs_index, mcs_table, n_re_per_rb, ues, None)
    }

    pub fn with_l2s(
        total_prbs: u16,
        fallback_mcs: u8,
        mcs_table: McsTable,
        n_re_per_rb: u32,
        ues: &[UeId],
        l2s: Option<Arc<L2sTables>>,
    ) -> Self {
        let queues = ues.iter().map(|&ue| UeQueue::new(ue)).collect::<Vec<_>>();
        let order = (0..queues.len()).collect();
        Self {
            total_prbs,
            fallback_mcs,
            mcs_table,
            n_re_per_rb,
            l2s,
            max_harq_attempts: DEFAULT_MAX_HARQ_ATTEMPTS,
            order,
            queues,
            completions: Vec::new(),
            in_flight_ue: Vec::new(),
            dropped_tbs: 0,
        }
    }

    #[allow(dead_code)]
    pub fn dropped_tbs(&self) -> u64 {
        self.dropped_tbs
    }

    fn index_of(&self, ue: UeId) -> Option<usize> {
        self.queues.iter().position(|q| q.ue == ue)
    }

    fn select_mcs(&self, qi: usize) -> u8 {
        match (&self.l2s, self.queues[qi].last_sinr) {
            (Some(tables), Some(sinr)) => tables.select_mcs(sinr),
            _ => self.fallback_mcs,
        }
    }

    #[inline]
    fn tb_budget(&self, mcs_index: u8) -> u64 {
        self.mcs_table
            .tbs(mcs_index, self.total_prbs as u32, self.n_re_per_rb, 1)
            .map(|b| b.value())
            .unwrap_or(0)
    }

    /// 新規 TB を引き当て、HARQ プロセスを生成する（初送）。
    /// `drained` に各パケットから引いた量を記録し、`remaining`/`backlog` を減らす。
    fn allocate_new(&mut self, qi: usize, mcs_index: u8, budget: u64) {
        let q = &mut self.queues[qi];
        let mut remaining_budget = budget;
        let mut drained: Vec<Bits> = Vec::new();

        for pkt in q.packets.iter_mut() {
            if remaining_budget == 0 {
                break;
            }
            let take = pkt.remaining.value().min(remaining_budget);
            if take == 0 {
                drained.push(Bits::ZERO);
                continue;
            }
            pkt.remaining = Bits::new(pkt.remaining.value() - take);
            q.backlog = Bits::new(q.backlog.value() - take);
            remaining_budget -= take;
            drained.push(Bits::new(take));
        }

        q.harq = Some(HarqProcess {
            drained,
            mcs_index,
            attempt: 0,
        });
    }

    /// ACK: HARQ プロセスを解放し、引き当て分で `remaining == 0` に達した
    /// 先頭パケットを完了として pop する（§15.2 動作規則 3）。
    fn commit_success(&mut self, qi: usize, completion: Slot) {
        let q = &mut self.queues[qi];
        q.harq = None;
        while let Some(front) = q.packets.front() {
            if front.remaining.value() == 0 {
                let done = q.packets.pop_front().unwrap();
                self.completions.push(PacketCompletion {
                    ue: q.ue,
                    bearer: done.bearer,
                    size: done.size,
                    arrival: done.arrival,
                    completion,
                });
            } else {
                break;
            }
        }
    }

    /// NACK: 最大試行未満なら再送待ち（attempt++、HARQ 保持）。
    /// 超過なら TB 破棄し引き当て分をキューへ戻す（再スケジュール対象に復帰）。
    fn handle_nack(&mut self, qi: usize) {
        let max = self.max_harq_attempts;
        let q = &mut self.queues[qi];
        // attempt を読み、必要なら drained を取り出して harq 借用を即座に切る。
        let drained = {
            let Some(harq) = q.harq.as_mut() else {
                return;
            };
            if harq.attempt + 1 < max {
                harq.attempt += 1; // 再送待ち（step が再送を発行）
                return;
            }
            // 最大再送超過: drained を奪って harq 借用を解放。
            std::mem::take(&mut harq.drained)
        };

        // 引き当て分をキューへ差し戻し、HARQ を解放。
        for (pkt, &d) in q.packets.iter_mut().zip(drained.iter()) {
            if d.value() == 0 {
                continue;
            }
            pkt.remaining = Bits::new(pkt.remaining.value() + d.value());
            q.backlog += d;
        }
        q.harq = None;
        self.dropped_tbs += 1;
    }

    /// この UE の grant を発行（新規 or 再送）。発行したら true。
    fn emit_grant(&mut self, qi: usize, out: &mut Vec<Grant>) -> bool {
        let ue = self.queues[qi].ue;
        // 再送待ち HARQ があれば最優先（同一 MCS、attempt を載せる）。
        // 借用を切るため必要値を先に Copy で取り出す。
        if let Some((mcs_index, attempt)) = self.queues[qi]
            .harq
            .as_ref()
            .map(|h| (h.mcs_index, h.attempt))
        {
            out.push(Grant {
                ue,
                prbs: PrbAllocation::new(0, self.total_prbs),
                mcs_index,
                direction: Direction::Downlink,
                harq_process: 0,
                harq_attempt: attempt,
            });
            self.in_flight_ue.push(ue);
            return true;
        }
        // 新規 TB。
        if self.queues[qi].backlog.value() > 0 {
            let mcs_index = self.select_mcs(qi);
            let budget = self.tb_budget(mcs_index);
            self.allocate_new(qi, mcs_index, budget);
            out.push(Grant {
                ue,
                prbs: PrbAllocation::new(0, self.total_prbs),
                mcs_index,
                direction: Direction::Downlink,
                harq_process: 0,
                harq_attempt: 0,
            });
            self.in_flight_ue.push(ue);
            return true;
        }
        false
    }
}

impl Mac for RoundRobinMac {
    fn enqueue(&mut self, ctx: &SlotContext, arrivals: &[TrafficArrival]) {
        for a in arrivals {
            if let Some(i) = self.index_of(a.ue) {
                self.queues[i].push(a.bearer, a.size, ctx.elapsed);
            }
        }
    }

    fn step(&mut self, _ctx: &SlotContext, out: &mut Vec<Grant>) {
        self.in_flight_ue.clear();

        // ラウンドロビン: 1 スロット 1 grant（既存挙動を維持）。再送待ち UE も
        // 通常の順番で巡回し、has_work（再送 or backlog）なら発行する。
        let n = self.order.len();
        for _ in 0..n {
            let i = self.order.pop_front().unwrap();
            self.order.push_back(i);
            if self.queues[i].has_work() && self.emit_grant(i, out) {
                return;
            }
        }
    }

    fn on_result(&mut self, ctx: &SlotContext, result: &TransportResult) {
        let Some(qi) = self.index_of(result.ue) else {
            return;
        };
        // ILLA フィードバック: 合成後実効 SINR を次回 MCS 選択へ。
        self.queues[qi].last_sinr = Some(result.effective_sinr);

        // 送信中だった UE のみ HARQ 遷移（in_flight に無ければ無視）。
        if !self.in_flight_ue.iter().any(|&u| u == result.ue) {
            return;
        }
        if result.success {
            self.commit_success(qi, ctx.elapsed);
        } else {
            self.handle_nack(qi);
        }
    }

    fn drain_completions(&mut self, out: &mut Vec<PacketCompletion>) {
        out.append(&mut self.completions);
    }
}
