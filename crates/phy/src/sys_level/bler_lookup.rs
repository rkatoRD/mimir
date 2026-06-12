use nr_core::Db;

fn required_sinr_db(mcs_index: u8) -> f64 {
    let lo = -5.0;
    let hi = 22.0;
    let max_idx = 28.0;
    let idx = (mcs_index as f64).min(max_idx);
    lo + (hi - lo) * (idx / max_idx)
}

pub fn block_error_rate(effective_sinr: Db, mcs_index: u8) -> f64 {
    let required = required_sinr_db(mcs_index);
    let delta = effective_sinr.value() - required;
    let steepness = 1.5;
    1.0 / (1.0 + (steepness * delta).exp())
}
