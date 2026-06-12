use nr_core::Bits;
use nr_spec::tbs::compute_tbs;

use crate::common::mcs_util::McsConfig;

pub fn transport_block_size(mcs: &McsConfig, mcs_index: u8, n_prb: u32, n_re_per_rb: u32) -> Bits {
    let Some(entry) = mcs.entry(mcs_index) else {
        return Bits::ZERO;
    };
    if n_prb == 0 {
        return Bits::ZERO;
    }

    compute_tbs(
        n_re_per_rb,
        n_prb,
        entry.core_rate(),
        entry.modulation_order,
        1,
    )
}
