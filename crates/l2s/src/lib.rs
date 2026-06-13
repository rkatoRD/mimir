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
}

impl L2sTables {
    pub fn from_csv(path: &Path) -> Result<Self, L2sError> {
        Self::from_csv_with_grid(
            path,
            DEFAULT_SINR_MIN_DB,
            DEFAULT_SINR_MAX_DB,
            DEFAULT_STEP_DB,
        )
    }

    pub fn from_csv_with_grid(
        path: &Path,
        sinr_min_db: f64,
        sinr_max_db: f64,
        step_db: f64,
    ) -> Result<Self, L2sError> {
        let text = fs::read_to_string(path)?;
        let thresholds = parse_format_b(&text)?;
        Ok(Self::build(thresholds, sinr_min_db, sinr_max_db, step_db))
    }

    fn build(
        mut thresholds: Vec<McsThreshold>,
        sinr_min_db: f64,
        sinr_max_db: f64,
        step_db: f64,
    ) -> Self {
        thresholds.sort_by(|a, b| a.min_sinr_db.partial_cmp(&b.min_sinr_db).unwrap());

        let min_mcs = thresholds.iter().map(|t| t.mcs).min().unwrap_or(0);

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