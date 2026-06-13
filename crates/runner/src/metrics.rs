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
        if self.count == 0 {
            0
        } else {
            self.min
        }
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
}

impl Default for LatencyStats {
    fn default() -> Self {
        Self::new()
    }
}