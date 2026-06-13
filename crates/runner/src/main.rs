mod mac;
mod metrics;
mod traffic;

use std::path::Path;
use std::sync::Arc;

use channel::Local5gChannel;
use engine::SimulatorBuilder;
use l2s::L2sTables;
use nr_core::{CellId, Hz, Meter, Point, UeId, Watt};
use nr_spec::{McsTable, Numerology};
use phy::SysPhy;
use sap::PacketCompletion;

use mac::RoundRobinMac;
use metrics::LatencyStats;
use traffic::ConstantTraffic;

const SEED: u64 = 0xC0FFEE;
const NUMEROLOGY_MU: u8 = 1;
const TOTAL_PRBS: u16 = 273;
const BANDWIDTH_HZ: f64 = 100.0e6;
const CARRIER_HZ: f64 = 4.85e9;
const TX_POWER_W: f64 = 40.0;
const MCS_INDEX: u8 = 16;
const BITS_PER_SLOT_PER_UE: u64 = 4096;
const TB_CAPACITY_BITS: u64 = 8192;
const N_SLOTS: u64 = 1000;
const L2S_CSV_PATH: &str = "data/l2s/mcs_mapping.csv";

struct CellPlan {
    id: CellId,
    position: Point,
    ues: Vec<(UeId, Point)>,
}

fn m(x: f64) -> Meter {
    Meter::new(x)
}

fn main() {
    let cell0 = CellPlan {
        id: CellId::new(0),
        position: Point::new(m(0.0), m(0.0), m(25.0)),
        ues: vec![
            (UeId::new(0), Point::new(m(50.0), m(0.0), m(1.5))),
            (UeId::new(1), Point::new(m(-30.0), m(40.0), m(1.5))),
        ],
    };
    let cell1 = CellPlan {
        id: CellId::new(1),
        position: Point::new(m(500.0), m(0.0), m(25.0)),
        ues: vec![
            (UeId::new(2), Point::new(m(450.0), m(0.0), m(1.5))),
            (UeId::new(3), Point::new(m(530.0), m(40.0), m(1.5))),
        ],
    };
    let cells = [cell0, cell1];

    let numerology = Numerology::new(NUMEROLOGY_MU);
    let bandwidth = Hz::new(BANDWIDTH_HZ);
    let channel = Local5gChannel::with_defaults(Hz::new(CARRIER_HZ));

    // CSV 由来 ILLA/BLER テーブル（設計 §15.3）。ロード失敗時は固定 MCS と
    // ロジスティック近似へフォールバック。同一 Arc を MAC（消費点 1）と
    // SysPhy（消費点 2）で共有し、MCS 選択と成否判定を構造的に整合させる。
    let l2s = match L2sTables::from_csv(Path::new(L2S_CSV_PATH)) {
        Ok(tables) => {
            println!("loaded L2S table     : {L2S_CSV_PATH}");
            Some(Arc::new(tables))
        }
        Err(e) => {
            eprintln!("warning: L2S load failed ({e}); using fixed MCS {MCS_INDEX}");
            None
        }
    };

    let sys_phy = SysPhy::with_l2s(McsTable::Table2, 120, l2s.clone());

    let mut builder =
        SimulatorBuilder::new(numerology, bandwidth, TOTAL_PRBS, SEED, channel, sys_phy);

    let mut cell_ues: Vec<(CellId, Vec<UeId>)> = Vec::new();

    for plan in &cells {
        let ue_ids: Vec<UeId> = plan.ues.iter().map(|(id, _)| *id).collect();
        let mac = RoundRobinMac::with_l2s(
            TOTAL_PRBS,
            MCS_INDEX,
            TB_CAPACITY_BITS,
            &ue_ids,
            l2s.clone(),
        );
        builder = builder.add_cell(plan.id, plan.position, Watt::new(TX_POWER_W), Box::new(mac));
        for (ue, pos) in &plan.ues {
            builder = builder.add_ue(*ue, plan.id, *pos);
        }
        cell_ues.push((plan.id, ue_ids));
    }

    let mut sim = builder.build();

    let mut traffic = ConstantTraffic::new(BITS_PER_SLOT_PER_UE);

    let mut total_tb_bits: u64 = 0;
    let mut delivered_bits: u64 = 0;
    let mut tb_count: u64 = 0;
    let mut tb_failures: u64 = 0;

    let mut latency = LatencyStats::new();
    let mut completion_buf: Vec<PacketCompletion> = Vec::new();

    for _ in 0..N_SLOTS {
        for (cell, ues) in &cell_ues {
            sim.generate_and_enqueue_traffic(*cell, &mut traffic, ues);
        }

        sim.step();

        for r in sim.last_results() {
            tb_count += 1;
            total_tb_bits += r.tb_size.value();
            if r.success {
                delivered_bits += r.tb_size.value();
            } else {
                tb_failures += 1;
            }
        }

        completion_buf.clear();
        sim.drain_completions(&mut completion_buf);
        latency.ingest(&completion_buf);
    }

    let slot_dur = numerology.slot_duration().value();
    let sim_time = slot_dur * N_SLOTS as f64;
    let throughput_mbps = (delivered_bits as f64) / sim_time / 1e6;
    let bler = if tb_count > 0 {
        tb_failures as f64 / tb_count as f64
    } else {
        0.0
    };

    println!("=== Mimir minimum viable operation ===");
    println!("slots simulated      : {N_SLOTS}");
    println!("sim time             : {sim_time:.4} s");
    println!("transport blocks     : {tb_count}");
    println!("TB failures          : {tb_failures}");
    println!("average BLER         : {bler:.4}");
    println!("offered TB bits      : {total_tb_bits}");
    println!("delivered bits       : {delivered_bits}");
    println!("aggregate throughput : {throughput_mbps:.3} Mbps");

    println!("--- packet latency (slots) ---");
    println!("completed packets    : {}", latency.count());
    if latency.count() > 0 {
        let to_ms = |slots: f64| slots * slot_dur * 1e3;
        println!(
            "mean latency         : {:.2} slots ({:.3} ms)",
            latency.mean(),
            to_ms(latency.mean())
        );
        println!("std dev              : {:.2} slots", latency.std_dev());
        println!(
            "min / max            : {} / {} slots",
            latency.min(),
            latency.max()
        );
        println!(
            "P50 / P95 / P99      : {} / {} / {} slots",
            latency.percentile(0.50),
            latency.percentile(0.95),
            latency.percentile(0.99)
        );
    }
}