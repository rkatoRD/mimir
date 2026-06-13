use nr_core::Bits;

#[derive(Debug, Clone, Copy)]
pub struct McsEntry {
    pub modulation_order: u8,
    /// 目標コードレート × 1024（3GPP TS 38.214 の表記そのまま）。
    /// Table 2 の index 20 (682.5) / 26 (916.5) は非整数のため f64 で保持する。
    pub target_code_rate_x1024: f64,
}

impl McsEntry {
    #[inline]
    pub fn code_rate(self) -> f64 {
        self.target_code_rate_x1024 / 1024.0
    }

    #[inline]
    pub fn spectral_efficiency(self) -> f64 {
        self.modulation_order as f64 * self.code_rate()
    }
}

const fn e(qm: u8, r_x1024: f64) -> McsEntry {
    McsEntry {
        modulation_order: qm,
        target_code_rate_x1024: r_x1024,
    }
}

pub const MCS_TABLE_1: [McsEntry; 29] = [
    e(2, 120.0),
    e(2, 157.0),
    e(2, 193.0),
    e(2, 251.0),
    e(2, 308.0),
    e(2, 379.0),
    e(2, 449.0),
    e(2, 526.0),
    e(2, 602.0),
    e(2, 679.0),
    e(4, 340.0),
    e(4, 378.0),
    e(4, 434.0),
    e(4, 490.0),
    e(4, 553.0),
    e(4, 616.0),
    e(4, 658.0),
    e(6, 438.0),
    e(6, 466.0),
    e(6, 517.0),
    e(6, 567.0),
    e(6, 616.0),
    e(6, 666.0),
    e(6, 719.0),
    e(6, 772.0),
    e(6, 822.0),
    e(6, 873.0),
    e(6, 910.0),
    e(6, 948.0),
];

pub const MCS_TABLE_2: [McsEntry; 28] = [
    e(2, 120.0),
    e(2, 193.0),
    e(2, 308.0),
    e(2, 449.0),
    e(2, 602.0),
    e(4, 378.0),
    e(4, 434.0),
    e(4, 490.0),
    e(4, 553.0),
    e(4, 616.0),
    e(4, 658.0),
    e(6, 466.0),
    e(6, 517.0),
    e(6, 567.0),
    e(6, 616.0),
    e(6, 666.0),
    e(6, 719.0),
    e(6, 772.0),
    e(6, 822.0),
    e(6, 873.0),
    e(8, 682.5),
    e(8, 711.0),
    e(8, 754.0),
    e(8, 797.0),
    e(8, 841.0),
    e(8, 885.0),
    e(8, 916.5),
    e(8, 948.0),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McsTable {
    /// Table 5.1.3.1-1 (Max 64QAM)。
    Table1,
    /// Table 5.1.3.1-2 (Max 256QAM)。
    Table2,
}

impl McsTable {
    #[inline]
    pub fn entry(self, mcs_index: u8) -> Option<McsEntry> {
        match self {
            McsTable::Table1 => MCS_TABLE_1.get(mcs_index as usize).copied(),
            McsTable::Table2 => MCS_TABLE_2.get(mcs_index as usize).copied(),
        }
    }

    pub fn tbs(self, mcs_index: u8, n_prb: u32, n_re_per_rb: u32, num_layers: u8) -> Option<Bits> {
        let entry = self.entry(mcs_index)?;
        if n_prb == 0 {
            return Some(Bits::ZERO);
        }
        Some(crate::tbs::compute_tbs(
            n_re_per_rb,
            n_prb,
            entry.code_rate(),
            entry.modulation_order,
            num_layers,
        ))
    }
}