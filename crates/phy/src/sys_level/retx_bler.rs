use nr_core::Db;

use super::bler_lookup;

pub fn residual_bler(effective_sinr: Db, mcs_index: u8, n_retx: u8) -> f64 {
    let single = bler_lookup::block_error_rate(effective_sinr, mcs_index);
    single.powi(n_retx as i32 + 1)
}
