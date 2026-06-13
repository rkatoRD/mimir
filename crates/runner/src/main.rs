mod mac;
mod metrics;
mod traffic;

use std::path::Path;
use std::sync::Arc;

use rayon::prelude::*;

use channel::Local5gChannel;
use engine::SimulatorBuilder;
use l2s::L2sTables;
use nr_core::{Bits, CellId, Hz, Meter, Point, UeId, Watt};
use nr_spec::{McsTable, Numerology};
use phy::SysPhy;
use sap::PacketCompletion;

use mac::RoundRobinMac;
use metrics::{LatencyStats, RunMetrics, TrialAggregate};
use traffic::{OuParams, OuPoissonTraffic, Positivity};

const BASE_SEED: u64 = 0xC0FFEE;
const N_TRIALS: u64 = 32; // Monte Carlo 試行数（シード列）。
const NUMEROLOGY_MU: u8 = 1;
const TOTAL_PRBS: u16 = 273;
const BANDWIDTH_HZ: f64 = 100.0e6;
const CARRIER_HZ: f64 = 4.85e9;
const TX_POWER_W: f64 = 40.0;
const MCS_INDEX: u8 = 16; // L2S 不在時のフォールバック固定 MCS。
// PHY と MAC で共有する数表設定（TBS 算出の整合に必須）。
const MCS_TABLE: McsTable = McsTable::Table2;
const N_RE_PER_RB: u32 = 120;
const OU_THETA: f64 = 5.0; // 回帰速度 [1/s]（全シナリオ共通）
const N_SLOTS: u64 = 1000;
const L2S_CSV_PATH: &str = "data/l2s/mcs_mapping.csv";

// 実測した 1 セル容量（高 SINR → ILLA は常時 MCS27, TBS≈241.7 kbit, slot 0.5ms）
// ≈ 483 Mbps。負荷率はこれを分母に逆算する。
struct Scenario {
    label: &'static str,
    note: &'static str,
    mu: f64,
    sigma: f64,
    packet_size_bits: u64,
}

const SCENARIOS: &[Scenario] = &[
    Scenario {
        label: "baseline (light)",
        note: "従来設定・軽負荷",
        mu: 1500.0,
        sigma: 1000.0,
        packet_size_bits: 50_000,
    },
    Scenario {
        label: "A: high load",
        note: "中サイズ・高頻度。1 パケット < TBS で 1 スロット完了",
        mu: 1500.0,
        sigma: 1000.0,
        packet_size_bits: 110_000,
    },
    Scenario {
        label: "B: large packets",
        note: "TBS(242kbit)超の大パケット→複数スロット送信",
        mu: 700.0,
        sigma: 500.0,
        packet_size_bits: 300_000,
    },
];

struct CellPlan {
    id: CellId,
    position: Point,
    ues: Vec<(UeId, Point)>,
}

fn m(x: f64) -> Meter {
    Meter::new(x)
}

fn cell_plans() -> Vec<CellPlan> {
    vec![
        CellPlan {
            id: CellId::new(0),
            position: Point::new(m(0.0), m(0.0), m(25.0)),
            ues: vec![
                (UeId::new(0), Point::new(m(50.0), m(0.0), m(1.5))),
                (UeId::new(1), Point::new(m(-30.0), m(40.0), m(1.5))),
            ],
        },
        CellPlan {
            id: CellId::new(1),
            position: Point::new(m(500.0), m(0.0), m(25.0)),
            ues: vec![
                (UeId::new(2), Point::new(m(450.0), m(0.0), m(1.5))),
                (UeId::new(3), Point::new(m(530.0), m(40.0), m(1.5))),
            ],
        },
    ]
}

/// 1 試行（1 シード）を独立に実行し、遅延統計とスカラー指標を返す。
/// 状態共有ゼロ（自前 EventLoop + トラフィックモデルを所有）→ ロックゼロ。
/// 同一シードは常に同一結果（決定論）。
fn run_once(
    scenario: &Scenario,
    seed: u64,
    l2s: &Option<Arc<L2sTables>>,
) -> (LatencyStats, RunMetrics) {
    let numerology = Numerology::new(NUMEROLOGY_MU);
    let bandwidth = Hz::new(BANDWIDTH_HZ);
    let slot_duration = numerology.slot_duration();
    let channel = Local5gChannel::with_defaults(Hz::new(CARRIER_HZ));
    let sys_phy = SysPhy::with_l2s(MCS_TABLE, N_RE_PER_RB, l2s.clone());

    let plans = cell_plans();
    let mut builder =
        SimulatorBuilder::new(numerology, bandwidth, TOTAL_PRBS, seed, channel, sys_phy);

    let mut cell_ues: Vec<(CellId, Vec<UeId>)> = Vec::new();
    for plan in &plans {
        let ue_ids: Vec<UeId> = plan.ues.iter().map(|(id, _)| *id).collect();
        let mac = RoundRobinMac::with_l2s(
            TOTAL_PRBS,
            MCS_INDEX,
            MCS_TABLE,
            N_RE_PER_RB,
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

    let ou_params = OuParams {
        theta: OU_THETA,
        mu: scenario.mu,
        sigma: scenario.sigma,
        packet_size: Bits::new(scenario.packet_size_bits),
        update_period: 1,
        positivity: Positivity::Clamp,
    };
    let mut cell_traffic: Vec<OuPoissonTraffic> = cell_ues
        .iter()
        .map(|(_, ues)| OuPoissonTraffic::new(ou_params, slot_duration, ues.len()))
        .collect();

    let mut delivered_bits: u64 = 0;
    let mut tb_count: u64 = 0;
    let mut tb_failures: u64 = 0;

    let mut latency = LatencyStats::new();
    let mut completion_buf: Vec<PacketCompletion> = Vec::new();

    for _ in 0..N_SLOTS {
        for (ci, (cell, ues)) in cell_ues.iter().enumerate() {
            sim.generate_and_enqueue_traffic(*cell, &mut cell_traffic[ci], ues);
        }

        sim.step();

        for r in sim.last_results() {
            tb_count += 1;
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

    let sim_time = slot_duration.value() * N_SLOTS as f64;
    let throughput_mbps = (delivered_bits as f64) / sim_time / 1e6;
    let bler = if tb_count > 0 {
        tb_failures as f64 / tb_count as f64
    } else {
        0.0
    };

    let metrics = RunMetrics {
        throughput_mbps,
        bler,
        tb_count,
        tb_failures,
        completed_packets: latency.count(),
        mean_latency_slots: latency.mean(),
    };
    (latency, metrics)
}

/// 1 シナリオを N_TRIALS 試行、rayon で試行間並列実行する（設計 §8.2）。
fn run_scenario(scenario: &Scenario, l2s: &Option<Arc<L2sTables>>) {
    // 試行間並列。各試行は独立シード BASE_SEED ^ trial。結果は試行 index 順に
    // 収集してから合成するため、合成順序は常に同一（決定論）。
    let mut results: Vec<(LatencyStats, RunMetrics)> = (0..N_TRIALS)
        .into_par_iter()
        .map(|trial| {
            let seed = BASE_SEED ^ trial.wrapping_mul(0x9E37_79B9_7F4A_7C15);
            run_once(scenario, seed, l2s)
        })
        .collect();

    // 遅延ヒストグラム/Welford を試行 index 順に合成（プール集計、全試行のパケットを
    // 1 つの分布として扱う）。並列 Welford は可換だが、index 順で固定しておく。
    let mut pooled = LatencyStats::new();
    let mut aggregate = TrialAggregate::new();
    for (lat, m) in results.drain(..) {
        pooled.merge(&lat);
        aggregate.ingest(&m);
    }

    print_report(scenario, &pooled, &aggregate);
}

fn print_report(scenario: &Scenario, pooled: &LatencyStats, aggregate: &TrialAggregate) {
    let slot_dur = Numerology::new(NUMEROLOGY_MU).slot_duration().value();
    let to_ms = |slots: f64| slots * slot_dur * 1e3;

    let (tp_mean, _tp_std, tp_ci) = aggregate.throughput_stats();
    let (bler_mean, _bler_std, bler_ci) = aggregate.bler_stats();
    let (lat_mean, lat_std, lat_ci) = aggregate.mean_latency_stats();

    println!("\n========================================================");
    println!("scenario             : {}", scenario.label);
    println!("  {}", scenario.note);
    println!("--------------------------------------------------------");
    println!(
        "traffic (OU-Poisson) : mu={} sigma={} pkt={} bits",
        scenario.mu, scenario.sigma, scenario.packet_size_bits
    );
    println!(
        "trials               : {} (parallel, seeds = BASE ^ trial·φ)",
        aggregate.n()
    );
    println!("--- per-trial aggregates (mean ± 95% CI) ---");
    println!("throughput           : {tp_mean:.2} ± {tp_ci:.2} Mbps");
    println!("BLER                 : {bler_mean:.4} ± {bler_ci:.4}");
    println!(
        "per-trial mean latency: {:.2} ± {:.2} slots (σ={:.2})",
        lat_mean, lat_ci, lat_std
    );
    println!("--- pooled packet latency over all trials (slots) ---");
    println!("completed packets    : {}", pooled.count());
    if pooled.count() > 0 {
        println!(
            "mean latency         : {:.2} slots ({:.3} ms)",
            pooled.mean(),
            to_ms(pooled.mean())
        );
        println!("std dev              : {:.2} slots", pooled.std_dev());
        println!(
            "min / max            : {} / {} slots",
            pooled.min(),
            pooled.max()
        );
        println!(
            "P50 / P95 / P99      : {} / {} / {} slots",
            pooled.percentile(0.50),
            pooled.percentile(0.95),
            pooled.percentile(0.99)
        );
    }
}

fn main() {
    // CSV 由来 ILLA/BLER テーブル（設計 §15.3）。ロード失敗時は固定 MCS と
    // ロジスティック近似へフォールバック。Arc は Send + Sync なので全試行で共有可能。
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

    println!("=== Mimir Monte Carlo: traffic scenario sweep ===");
    println!("slots/trial          : {N_SLOTS}");
    println!("trials/scenario      : {N_TRIALS}");
    println!("base seed            : {BASE_SEED:#x}");
    println!("threads              : {}", rayon::current_num_threads());

    for scenario in SCENARIOS {
        run_scenario(scenario, &l2s);
    }
}
