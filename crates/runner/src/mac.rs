//! 最小限の MAC 実装（policy 層）。
//!
//! バックログを持つ UE をラウンドロビンで選び、固定 MCS で全帯域 PRB を
//! 1 UE に割り当てる。スケジューラ／HARQ は持たない「最小動作」用。
//! Week 2 マイルストーン（2-cell, no-coordination, AWGN, fixed-UE）専用。

use std::collections::VecDeque;

use nr_core::{Bits, Direction, Slot, UeId};
use sap::{Grant, Mac, PacketCompletion, PrbAllocation, SlotContext, TrafficArrival, TransportResult};

struct UeBacklog {
    ue: UeId,
    backlog: Bits,
}

pub struct RoundRobinMac {
    total_prbs: u16,
    mcs_index: u8,
    order: VecDeque<usize>,
    backlogs: Vec<UeBacklog>,
    completions: Vec<PacketCompletion>,
}

impl RoundRobinMac {
    pub fn new(total_prbs: u16, mcs_index: u8, ues: &[UeId]) -> Self {
        let backlogs = ues
            .iter()
            .map(|&ue| UeBacklog {
                ue,
                backlog: Bits::ZERO,
            })
            .collect::<Vec<_>>();
        let order = (0..backlogs.len()).collect();
        Self {
            total_prbs,
            mcs_index,
            order,
            backlogs,
            completions: Vec::new(),
        }
    }

    fn index_of(&self, ue: UeId) -> Option<usize> {
        self.backlogs.iter().position(|b| b.ue == ue)
    }
}

impl Mac for RoundRobinMac {
    fn enqueue(&mut self, _ctx: &SlotContext, arrivals: &[TrafficArrival]) {
        for a in arrivals {
            if let Some(i) = self.index_of(a.ue) {
                self.backlogs[i].backlog += a.size;
            }
        }
    }

    fn step(&mut self, _ctx: &SlotContext, out: &mut Vec<Grant>) {
        let n = self.order.len();
        for _ in 0..n {
            let i = self.order.pop_front().unwrap();
            self.order.push_back(i);
            if self.backlogs[i].backlog.value() > 0 {
                out.push(Grant {
                    ue: self.backlogs[i].ue,
                    prbs: PrbAllocation::new(0, self.total_prbs),
                    mcs_index: self.mcs_index,
                    direction: Direction::Downlink,
                });
                return;
            }
        }
    }

    fn on_result(&mut self, _ctx: &SlotContext, result: &TransportResult) {
        if !result.success {
            return;
        }
        if let Some(i) = self.index_of(result.ue) {
            let b = &mut self.backlogs[i];
            let drained = result.tb_size.value().min(b.backlog.value());
            b.backlog = Bits::new(b.backlog.value() - drained);
            self.completions.push(PacketCompletion {
                ue: result.ue,
                bearer: nr_core::BearerId::new(0),
                size: Bits::new(drained),
                arrival: Slot::new(0),
                completion: Slot::new(0),
            });
        }
    }

    fn drain_completions(&mut self, out: &mut Vec<PacketCompletion>) {
        out.append(&mut self.completions);
    }
}