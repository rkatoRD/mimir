//! パケットキュー MAC + HARQ 状態機械（policy 層、設計 §15.2 / §4.3）。
//!
//! 方針 Y（Scheduler 分離）後の MAC の責務は **キュー管理・HARQ 状態機械・
//! TB 引き当て/確定**に限定される。UE 選択・PRB 配分・MCS 選択(ILLA) は
//! [`Scheduler`] が担い、MAC は各 UE の状態を [`SchedulingRequest`] に集約して
//! スケジューラへ渡し、返ってきた [`Grant`] に対して TB を引き当てる。
//!
//! MAC↔Scheduler 連携: MAC が `Scheduler` を内部に保持し、`Mac::step` 内で
//! 1) 各 UE の SchedulingRequest を組み立て（再送待ち HARQ は forced_mcs を載せる）、
//! 2) `scheduler.schedule` で Grant を得て、
//! 3) Grant の PRB 数から実 TBS を求め TB を引き当て HARQ プロセスを更新する。
//! これにより Mac トレイト（sap 契約）は無変更のまま Scheduler が差し替え可能。
//!
//! HARQ（設計 §15.2）:
//! - TB ↔ パケット集合の対応はグラント発行時に確定し、HARQ プロセスが `drained` を保持。
//! - NACK で再送待ち（attempt++）、ACK で確定、最大試行超過で引き当て分をキューへ戻す。
//! - PHY は `harq_attempt` で再送合成利得を評価する（方式 A）。

use std::collections::VecDeque;

use nr_core::{BearerId, Bits, Db, Slot, UeId};
use nr_spec::McsTable;
use sap::{
    Grant, Mac, PacketCompletion, Scheduler, SchedulingRequest, SlotContext, TrafficArrival,
    TransportResult,
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
    drained: Vec<Bits>,
    mcs_index: u8,
    attempt: u8,
}

struct UeQueue {
    ue: UeId,
    packets: VecDeque<QueuedPacket>,
    backlog: Bits,
    last_sinr: Option<Db>,
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
}

/// MAC。スケジューラを内部に保持し、キューと HARQ を管理する。
pub struct QueueMac<S: Scheduler> {
    scheduler: S,
    /// TB ビット予算算出用の数表（PHY と同一 → 引き当て量と配送量が一致）。
    mcs_table: McsTable,
    n_re_per_rb: u32,
    max_harq_attempts: u8,
    queues: Vec<UeQueue>,
    completions: Vec<PacketCompletion>,
    /// このスロットで組み立てる SchedulingRequest の再利用バッファ（確保ゼロ、§5.4）。
    req_buf: Vec<SchedulingRequest>,
    /// このスロットでスケジューラが返した Grant の再利用バッファ。
    grant_buf: Vec<Grant>,
    /// このスロットで送信した UE（on_result と突き合わせる）。
    in_flight_ue: Vec<UeId>,
    dropped_tbs: u64,
}

impl<S: Scheduler> QueueMac<S> {
    pub fn new(scheduler: S, mcs_table: McsTable, n_re_per_rb: u32, ues: &[UeId]) -> Self {
        let queues = ues.iter().map(|&ue| UeQueue::new(ue)).collect::<Vec<_>>();
        Self {
            scheduler,
            mcs_table,
            n_re_per_rb,
            max_harq_attempts: DEFAULT_MAX_HARQ_ATTEMPTS,
            queues,
            completions: Vec::new(),
            req_buf: Vec::new(),
            grant_buf: Vec::new(),
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

    /// Grant の PRB 数と MCS から 1 スロットの TB ビット予算（実 TBS）を求める。
    /// PHY の `evaluate` と同じ数表・同じ引数（grant.prbs.count）で計算するため、
    /// 引き当て量と配送量が構造的に一致する（スループットとキューの整合）。
    #[inline]
    fn tb_budget(&self, mcs_index: u8, prb_count: u16) -> u64 {
        self.mcs_table
            .tbs(mcs_index, prb_count as u32, self.n_re_per_rb, 1)
            .map(|b| b.value())
            .unwrap_or(0)
    }

    /// 新規 TB を引き当て、HARQ プロセスを生成する（初送）。
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

    fn handle_nack(&mut self, qi: usize) {
        let max = self.max_harq_attempts;
        let q = &mut self.queues[qi];
        let drained = {
            let Some(harq) = q.harq.as_mut() else {
                return;
            };
            if harq.attempt + 1 < max {
                harq.attempt += 1; // 再送待ち（次の step が再送要求を出す）
                return;
            }
            std::mem::take(&mut harq.drained)
        };

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
}

impl<S: Scheduler> Mac for QueueMac<S> {
    fn enqueue(&mut self, ctx: &SlotContext, arrivals: &[TrafficArrival]) {
        for a in arrivals {
            if let Some(i) = self.index_of(a.ue) {
                self.queues[i].push(a.bearer, a.size, ctx.elapsed);
            }
        }
    }

    fn step(&mut self, ctx: &SlotContext, out: &mut Vec<Grant>) {
        self.in_flight_ue.clear();

        // 1) 各 UE の SchedulingRequest を組み立てる（UeStore 順 = 決定論）。
        //    再送待ち HARQ は forced_mcs を載せた再送要求、それ以外は新規要求。
        self.req_buf.clear();
        for q in &self.queues {
            let req = if let Some(harq) = q.harq.as_ref() {
                SchedulingRequest::retransmission(
                    q.ue,
                    q.backlog,
                    q.last_sinr,
                    harq.mcs_index,
                    harq.attempt,
                )
            } else {
                SchedulingRequest::new_tx(q.ue, q.backlog, q.last_sinr)
            };
            self.req_buf.push(req);
        }

        // 2) スケジューラが UE 選択 + PRB 配分 + MCS 選択を行う。
        self.grant_buf.clear();
        self.scheduler
            .schedule(ctx, &self.req_buf, &mut self.grant_buf);

        // 3) 返った Grant に対し TB を引き当て、HARQ プロセスを更新する。
        for gi in 0..self.grant_buf.len() {
            let grant = self.grant_buf[gi];
            let Some(qi) = self.index_of(grant.ue) else {
                continue;
            };
            // 初送（HARQ 未保持）のみ新規引き当て。再送（HARQ 保持中）は
            // 既存の drained をそのまま使うので再引き当てしない。
            if self.queues[qi].harq.is_none() {
                let budget = self.tb_budget(grant.mcs_index, grant.prbs.count);
                self.allocate_new(qi, grant.mcs_index, budget);
            }
            self.in_flight_ue.push(grant.ue);
            out.push(grant);
        }
    }

    fn on_result(&mut self, ctx: &SlotContext, result: &TransportResult) {
        let Some(qi) = self.index_of(result.ue) else {
            return;
        };
        // ILLA フィードバック: 合成後実効 SINR を次回 MCS 選択へ。
        self.queues[qi].last_sinr = Some(result.effective_sinr);

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
