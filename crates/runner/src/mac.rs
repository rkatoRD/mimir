use std::collections::VecDeque;

use nr_core::{BearerId, Bits, Direction, Slot, UeId};
use sap::{Grant, Mac, PacketCompletion, PrbAllocation, SlotContext, TrafficArrival, TransportResult};

#[derive(Debug, Clone, Copy)]
struct QueuedPacket {
    bearer: BearerId,
    size: Bits,
    remaining: Bits,
    arrival: Slot,
}

struct UeQueue {
    ue: UeId,
    packets: VecDeque<QueuedPacket>,
    backlog: Bits,
}
 
impl UeQueue {
    fn new(ue: UeId) -> Self {
        Self {
            ue,
            packets: VecDeque::new(),
            backlog: Bits::ZERO,
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

struct PendingTb {
    ue: UeId,
    drained: Vec<Bits>,
}
 
pub struct RoundRobinMac {
    total_prbs: u16,
    mcs_index: u8,
    tb_capacity_bits: u64,
    order: VecDeque<usize>,
    queues: Vec<UeQueue>,
    pending: Vec<PendingTb>,
    completions: Vec<PacketCompletion>,
}
 
impl RoundRobinMac {
    pub fn new(total_prbs: u16, mcs_index: u8, tb_capacity_bits: u64, ues: &[UeId]) -> Self {
        let queues = ues.iter().map(|&ue| UeQueue::new(ue)).collect::<Vec<_>>();
        let order = (0..queues.len()).collect();
        Self {
            total_prbs,
            mcs_index,
            tb_capacity_bits,
            order,
            queues,
            pending: Vec::new(),
            completions: Vec::new(),
        }
    }
 
    fn index_of(&self, ue: UeId) -> Option<usize> {
        self.queues.iter().position(|q| q.ue == ue)
    }
 
    fn allocate(&mut self, qi: usize, budget: u64) -> PendingTb {
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
 
        PendingTb {
            ue: q.ue,
            drained,
        }
    }
 
    fn commit_success(&mut self, pending: &PendingTb, completion: Slot) {
        let Some(qi) = self.index_of(pending.ue) else {
            return;
        };
        let q = &mut self.queues[qi];
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
 
    fn rollback_failure(&mut self, pending: &PendingTb) {
        let Some(qi) = self.index_of(pending.ue) else {
            return;
        };
        let q = &mut self.queues[qi];
        for (pkt, &d) in q.packets.iter_mut().zip(pending.drained.iter()) {
            if d.value() == 0 {
                continue;
            }
            pkt.remaining = Bits::new(pkt.remaining.value() + d.value());
            q.backlog += d;
        }
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
        self.pending.clear();
 
        let n = self.order.len();
        for _ in 0..n {
            let i = self.order.pop_front().unwrap();
            self.order.push_back(i);
            if self.queues[i].backlog.value() > 0 {
                let ue = self.queues[i].ue;
                let pending = self.allocate(i, self.tb_capacity_bits);
                self.pending.push(pending);
                out.push(Grant {
                    ue,
                    prbs: PrbAllocation::new(0, self.total_prbs),
                    mcs_index: self.mcs_index,
                    direction: Direction::Downlink,
                });
                return;
            }
        }
    }
 
    fn on_result(&mut self, ctx: &SlotContext, result: &TransportResult) {
        let Some(pos) = self.pending.iter().position(|p| p.ue == result.ue) else {
            return;
        };
        let pending = self.pending.remove(pos);
        if result.success {
            self.commit_success(&pending, ctx.elapsed);
        } else {
            self.rollback_failure(&pending);
        }
    }
 
    fn drain_completions(&mut self, out: &mut Vec<PacketCompletion>) {
        out.append(&mut self.completions);
    }
}