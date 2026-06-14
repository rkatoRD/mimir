use sap::PacketCompletion;

const MAX_LATENCY_SLOTS: usize = 4096;

pub struct LatencyStats {
    count: u64,
    mean: f64,
    m2: f64,
    min: u64,
    max: u64,
    hist: Vec<u64>,
    overflow: u64,
}

impl LatencyStats {
    pub fn new() -> Self {
        Self {
            count: 0,
            mean: 0.0,
            m2: 0.0,
            min: u64::MAX,
            max: 0,
            hist: vec![0; MAX_LATENCY_SLOTS],
            overflow: 0,
        }
    }

    pub fn ingest(&mut self, completions: &[PacketCompletion]) {
        for c in completions {
            let latency = c.completion.value().saturating_sub(c.arrival.value());
            self.record(latency);
        }
    }

    fn record(&mut self, latency: u64) {
        self.count += 1;
        let x = latency as f64;
        let delta = x - self.mean;
        self.mean += delta / self.count as f64;
        let delta2 = x - self.mean;
        self.m2 += delta * delta2;

        self.min = self.min.min(latency);
        self.max = self.max.max(latency);

        if (latency as usize) < MAX_LATENCY_SLOTS {
            self.hist[latency as usize] += 1;
        } else {
            self.overflow += 1;
        }
    }

    pub fn count(&self) -> u64 {
        self.count
    }

    pub fn mean(&self) -> f64 {
        self.mean
    }

    pub fn variance(&self) -> f64 {
        if self.count > 1 {
            self.m2 / (self.count - 1) as f64
        } else {
            0.0
        }
    }

    pub fn std_dev(&self) -> f64 {
        self.variance().sqrt()
    }

    pub fn min(&self) -> u64 {
        if self.count == 0 { 0 } else { self.min }
    }

    pub fn max(&self) -> u64 {
        self.max
    }

    pub fn percentile(&self, q: f64) -> u64 {
        if self.count == 0 {
            return 0;
        }
        let target = (q * self.count as f64).ceil() as u64;
        let mut cum: u64 = 0;
        for (slot, &n) in self.hist.iter().enumerate() {
            cum += n;
            if cum >= target {
                return slot as u64;
            }
        }
        MAX_LATENCY_SLOTS as u64
    }

    /// 別の集計を取り込む（試行間並列の結果統合用）。
    /// 平均/分散は Chan らの並列 Welford 結合式で厳密に合成する。
    /// ヒストグラム・min/max・overflow は単純加算/比較。
    /// 結合順序に依存しない（可換・結合的）ため決定論を損なわない。
    pub fn merge(&mut self, other: &LatencyStats) {
        if other.count == 0 {
            return;
        }
        if self.count == 0 {
            self.count = other.count;
            self.mean = other.mean;
            self.m2 = other.m2;
            self.min = other.min;
            self.max = other.max;
            self.hist.clone_from(&other.hist);
            self.overflow = other.overflow;
            return;
        }

        let na = self.count as f64;
        let nb = other.count as f64;
        let delta = other.mean - self.mean;
        let total = na + nb;

        self.mean += delta * nb / total;
        self.m2 += other.m2 + delta * delta * na * nb / total;
        self.count += other.count;
        self.min = self.min.min(other.min);
        self.max = self.max.max(other.max);
        self.overflow += other.overflow;
        for (a, b) in self.hist.iter_mut().zip(other.hist.iter()) {
            *a += *b;
        }
    }
}

impl Default for LatencyStats {
    fn default() -> Self {
        Self::new()
    }
}

/// 1 試行（1 シード）のスカラー指標。試行間で平均・信頼区間を取る素材。
/// 一部フィールドは将来の per-trial CSV recorder（設計 §9）用に保持する。
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct RunMetrics {
    pub throughput_mbps: f64,
    pub bler: f64,
    pub tb_count: u64,
    pub tb_failures: u64,
    pub completed_packets: u64,
    pub mean_latency_slots: f64,
    /// 全 TB 評価の実効 SINR 平均 [dB]（HARQ 合成後の報告値）。
    pub mean_sinr_db: f64,
    /// 全 TB 評価の実効 SINR 最小 [dB]（セル端の最悪条件の指標）。
    pub min_sinr_db: f64,
    /// HARQ 再送 TB 数（harq_attempt > 0 の送信回数）。
    pub harq_retx: u64,
}

/// 試行間スカラーのオンライン集計（平均・標準偏差・95% 信頼区間）。
/// Monte Carlo の試行数に対して O(1) メモリ。
pub struct TrialAggregate {
    n: u64,
    // 各指標の Welford 累積（mean, m2）。
    throughput: (f64, f64),
    bler: (f64, f64),
    mean_latency: (f64, f64),
    mean_sinr: (f64, f64),
    // 最小 SINR の試行平均（プール最悪値ではなく試行毎最小の平均）。
    min_sinr: (f64, f64),
    // HARQ 再送数の試行合計。
    harq_retx_total: u64,
}

impl TrialAggregate {
    pub fn new() -> Self {
        Self {
            n: 0,
            throughput: (0.0, 0.0),
            bler: (0.0, 0.0),
            mean_latency: (0.0, 0.0),
            mean_sinr: (0.0, 0.0),
            min_sinr: (0.0, 0.0),
            harq_retx_total: 0,
        }
    }

    pub fn ingest(&mut self, m: &RunMetrics) {
        self.n += 1;
        welford(&mut self.throughput, self.n, m.throughput_mbps);
        welford(&mut self.bler, self.n, m.bler);
        welford(&mut self.mean_latency, self.n, m.mean_latency_slots);
        welford(&mut self.mean_sinr, self.n, m.mean_sinr_db);
        welford(&mut self.min_sinr, self.n, m.min_sinr_db);
        self.harq_retx_total += m.harq_retx;
    }

    pub fn n(&self) -> u64 {
        self.n
    }

    /// (平均, 標準偏差, 95% 信頼区間の半幅) を返す。
    pub fn throughput_stats(&self) -> (f64, f64, f64) {
        self.stats(self.throughput)
    }

    pub fn bler_stats(&self) -> (f64, f64, f64) {
        self.stats(self.bler)
    }

    pub fn mean_latency_stats(&self) -> (f64, f64, f64) {
        self.stats(self.mean_latency)
    }

    pub fn mean_sinr_stats(&self) -> (f64, f64, f64) {
        self.stats(self.mean_sinr)
    }

    pub fn min_sinr_stats(&self) -> (f64, f64, f64) {
        self.stats(self.min_sinr)
    }

    pub fn harq_retx_total(&self) -> u64 {
        self.harq_retx_total
    }

    fn stats(&self, acc: (f64, f64)) -> (f64, f64, f64) {
        let mean = acc.0;
        if self.n < 2 {
            return (mean, 0.0, 0.0);
        }
        let var = acc.1 / (self.n - 1) as f64;
        let std = var.sqrt();
        // 正規近似の 95% CI 半幅: 1.96 × std / √n。
        let ci = 1.96 * std / (self.n as f64).sqrt();
        (mean, std, ci)
    }
}

impl Default for TrialAggregate {
    fn default() -> Self {
        Self::new()
    }
}

#[inline]
fn welford(acc: &mut (f64, f64), n: u64, x: f64) {
    let delta = x - acc.0;
    acc.0 += delta / n as f64;
    let delta2 = x - acc.0;
    acc.1 += delta * delta2;
}
