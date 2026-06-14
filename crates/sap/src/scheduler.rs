use crate::messages::{Grant, SlotContext};
use nr_core::{Bits, Db, UeId};

/// スケジューラへの 1 UE 分の要求（設計 §6.5 メッセージ型 / §15.3 消費点 1）。
///
/// MAC が各 UE のキュー状態・チャネル品質・HARQ 状態を集約してスケジューラへ渡す。
/// スケジューラはこれを見て UE を選び、PRB を配分し、MCS を決めて [`Grant`] を作る。
///
/// HARQ 再送（`harq_retx == true`）の要求は、確定済み MCS（`forced_mcs`）を
/// そのまま使う必要がある（チェイス合成は同一 MCS 前提）。スケジューラは
/// 再送要求に対して MCS 選択（ILLA）をバイパスし、`forced_mcs` を採用する。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SchedulingRequest {
    pub ue: UeId,
    pub backlog: Bits,
    /// 実効 SINR フィードバック（ILLA の MCS 選択入力、設計 §15.3）。
    /// 未観測（初回）の UE は `None`。
    pub channel_quality: Option<Db>,
    /// HARQ 再送要求か。true なら `forced_mcs` を使い MCS 選択をバイパスする。
    pub harq_retx: bool,
    /// 再送試行回数（`Grant.harq_attempt` に載せる）。初送要求では 0。
    pub harq_attempt: u8,
    /// 再送時の確定 MCS（`harq_retx == true` のときのみ有効）。
    pub forced_mcs: u8,
}

impl SchedulingRequest {
    /// 新規（初送）要求を作る。MCS はスケジューラが `channel_quality` から決める。
    #[inline]
    pub fn new_tx(ue: UeId, backlog: Bits, channel_quality: Option<Db>) -> Self {
        Self {
            ue,
            backlog,
            channel_quality,
            harq_retx: false,
            harq_attempt: 0,
            forced_mcs: 0,
        }
    }

    /// HARQ 再送要求を作る。MCS は `forced_mcs` 固定（同一 MCS 再送）。
    #[inline]
    pub fn retransmission(
        ue: UeId,
        backlog: Bits,
        channel_quality: Option<Db>,
        forced_mcs: u8,
        harq_attempt: u8,
    ) -> Self {
        Self {
            ue,
            backlog,
            channel_quality,
            harq_retx: true,
            harq_attempt,
            forced_mcs,
        }
    }
}

pub trait Scheduler {
    /// 1 スロット分のスケジューリング。`requests` の中から UE を選び、PRB を
    /// 配分し、MCS を決めて `out` へ [`Grant`] を書き込む（out-parameter、§5.4）。
    ///
    /// HARQ 再送要求（`req.harq_retx`）には `req.forced_mcs` をそのまま使い、
    /// 新規要求には `req.channel_quality` から MCS を選ぶ（ILLA、§15.3 消費点 1）。
    fn schedule(&mut self, ctx: &SlotContext, requests: &[SchedulingRequest], out: &mut Vec<Grant>);
}
