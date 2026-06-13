use std::fs;
use std::path::Path;

use nr_core::Db;
const DEFAULT_SINR_MIN_DB: f64 = -10.0;
const DEFAULT_SINR_MAX_DB: f64 = 30.0;
const DEFAULT_STEP_DB: f64 = 0.1;

#[derive(Debug)]
pub enum L2sError {
    Io(std::io::Error),
    Parse { line: usize, reason: String },
    Empty,
}

impl std::fmt::Display for L2sError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            L2sError::Io(e) => write!(f, "I/O error: {e}"),
            L2sError::Parse { line, reason } => {
                write!(f, "parse error at line {line}: {reason}")
            }
            L2sError::Empty => write!(f, "no MCS rows found in CSV"),
        }
    }
}

impl std::error::Error for L2sError {}

impl From<std::io::Error> for L2sError {
    fn from(e: std::io::Error) -> Self {
        L2sError::Io(e)
    }
}

#[derive(Debug, Clone, Copy)]
struct McsThreshold {
    mcs: u8,
    min_sinr_db: f64,
}

pub struct L2sTables {
    sinr_min_db: f64,
    inv_step: f64,
    n_bins: usize,
    mcs_select: Vec<u8>,
    min_mcs: u8,
    /// 形式 B のしきい値 SINR を MCS index 直引きで保持する。
    /// `threshold[mcs]` = その MCS が目標 BLER を満たす最小 SINR [dB]。
    /// CSV に無い MCS は `None`（疎な MCS 集合に対応）。消費点 2（phy/sys）が
    /// しきい値中心の BLER 評価を組むために参照する（設計 §15.3）。
    threshold_db: Vec<Option<f64>>,
    /// 形式 B が表す目標 BLER（しきい値の定義点）。既定 0.1。
    target_bler: f64,
}

const DEFAULT_TARGET_BLER: f64 = 0.1;

impl L2sTables {
    pub fn from_csv(path: &Path) -> Result<Self, L2sError> {
        Self::from_csv_with_grid(
            path,
            DEFAULT_SINR_MIN_DB,
            DEFAULT_SINR_MAX_DB,
            DEFAULT_STEP_DB,
            DEFAULT_TARGET_BLER,
        )
    }

    pub fn from_csv_with_grid(
        path: &Path,
        sinr_min_db: f64,
        sinr_max_db: f64,
        step_db: f64,
        target_bler: f64,
    ) -> Result<Self, L2sError> {
        let text = fs::read_to_string(path)?;
        let thresholds = parse_format_b(&text)?;
        Ok(Self::build(
            thresholds,
            sinr_min_db,
            sinr_max_db,
            step_db,
            target_bler,
        ))
    }

    fn build(
        mut thresholds: Vec<McsThreshold>,
        sinr_min_db: f64,
        sinr_max_db: f64,
        step_db: f64,
        target_bler: f64,
    ) -> Self {
        thresholds.sort_by(|a, b| a.min_sinr_db.partial_cmp(&b.min_sinr_db).unwrap());

        let min_mcs = thresholds.iter().map(|t| t.mcs).min().unwrap_or(0);
        let max_mcs = thresholds.iter().map(|t| t.mcs).max().unwrap_or(0);

        // MCS index 直引きのしきい値配列。CSV に無い MCS は None。
        let mut threshold_db = vec![None; max_mcs as usize + 1];
        for t in &thresholds {
            threshold_db[t.mcs as usize] = Some(t.min_sinr_db);
        }

        let span = (sinr_max_db - sinr_min_db).max(step_db);
        let n_bins = (span / step_db).ceil() as usize + 1;
        let inv_step = 1.0 / step_db;

        let mut mcs_select = vec![min_mcs; n_bins];
        for bin in 0..n_bins {
            let sinr = sinr_min_db + bin as f64 * step_db;
            let mut chosen = min_mcs;
            for t in &thresholds {
                if t.min_sinr_db <= sinr {
                    chosen = t.mcs;
                } else {
                    break;
                }
            }
            mcs_select[bin] = chosen;
        }

        Self {
            sinr_min_db,
            inv_step,
            n_bins,
            mcs_select,
            min_mcs,
            threshold_db,
            target_bler,
        }
    }

    #[inline]
    pub fn select_mcs(&self, sinr: Db) -> u8 {
        let bin = (((sinr.value() - self.sinr_min_db) * self.inv_step) as isize)
            .clamp(0, self.n_bins as isize - 1) as usize;
        self.mcs_select[bin]
    }

    pub fn min_mcs(&self) -> u8 {
        self.min_mcs
    }

    /// 形式 B のしきい値 SINR [dB]（その MCS が目標 BLER を満たす最小 SINR）。
    /// CSV に存在しない MCS は `None`。
    #[inline]
    pub fn threshold_sinr_db(&self, mcs: u8) -> Option<f64> {
        self.threshold_db.get(mcs as usize).copied().flatten()
    }

    /// しきい値の定義点である目標 BLER（既定 0.1）。
    #[inline]
    pub fn target_bler(&self) -> f64 {
        self.target_bler
    }

    /// しきい値中心のロジスティック近似による BLER（消費点 2 = phy/sys）。
    ///
    /// 形式 B はしきい値（BLER = target を満たす最小 SINR）のみを与えるため、
    /// 厳密な BLER 曲線は持たない。ここでは各 MCS のしきい値 SINR を
    /// 「BLER = target となる点」に固定したロジスティック曲線を構成する。
    /// これにより ILLA（消費点 1）が選んだ MCS のしきい値 SINR で PHY の
    /// BLER がちょうど target になり、選択と成否判定が同一データ上で整合する
    /// （設計 §15.3 の構造的目的）。`steepness` は曲線の急峻さ [1/dB]。
    /// しきい値未登録の MCS では `None` を返し、呼び出し側がフォールバックする。
    #[inline]
    pub fn bler(&self, sinr: Db, mcs: u8, steepness: f64) -> Option<f64> {
        let thr = self.threshold_sinr_db(mcs)?;
        // logit(target) を満たすようオフセット。delta = sinr - thr。
        // delta = 0（しきい値上）で bler = target になるよう定数項を解く。
        // bler(delta) = 1 / (1 + exp(steepness*delta + c)),  c = ln(target/(1-target))^{-1}... を
        // target = 1/(1+exp(c)) すなわち c = ln((1-target)/target) で固定。
        let c = ((1.0 - self.target_bler) / self.target_bler).ln();
        let delta = sinr.value() - thr;
        Some(1.0 / (1.0 + (steepness * delta + c).exp()))
    }
}

fn parse_format_b(text: &str) -> Result<Vec<McsThreshold>, L2sError> {
    let mut out = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let mut fields = line.split(',');
        let first = fields.next().unwrap_or("").trim();
        if first.parse::<u8>().is_err() {
            continue;
        }
        let mcs: u8 = first.parse().map_err(|_| L2sError::Parse {
            line: i + 1,
            reason: format!("invalid MCS index: {first:?}"),
        })?;
        let snr_str = fields.next().unwrap_or("").trim();
        let min_sinr_db: f64 = snr_str.parse().map_err(|_| L2sError::Parse {
            line: i + 1,
            reason: format!("invalid SNR_dB: {snr_str:?}"),
        })?;
        out.push(McsThreshold { mcs, min_sinr_db });
    }
    if out.is_empty() {
        return Err(L2sError::Empty);
    }
    Ok(out)
}