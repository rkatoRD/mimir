use nr_core::Bits;

#[derive(Debug, Clone, Copy)]
pub struct McsEntry {
    pub modulation_order: u8,
    pub target_code_rate_x1024: u16,
}

impl McsEntry {
    pub fn code_rate(self) -> f64 {
        self.target_code_rate_x1024 as f64 / 1024.0
    }

    pub fn spectral_efficiency(self) -> f64 {
        self.modulation_order as f64 * self.code_rate()
    }
}

const fn e(qm: u8, r_x1024: u16) -> McsEntry {
    McsEntry {
        modulation_order: qm,
        target_code_rate_x1024: r_x1024,
    }
}

pub const MCS_TABLE_1: [McsEntry; 29] = [
    e(2, 120),
    e(2, 157),
    e(2, 193),
    e(2, 251),
    e(2, 308),
    e(2, 379),
    e(2, 449),
    e(2, 526),
    e(2, 602),
    e(2, 679),
    e(4, 340),
    e(4, 378),
    e(4, 434),
    e(4, 490),
    e(4, 553),
    e(4, 616),
    e(4, 658),
    e(6, 438),
    e(6, 466),
    e(6, 517),
    e(6, 567),
    e(6, 616),
    e(6, 666),
    e(6, 719),
    e(6, 772),
    e(6, 822),
    e(6, 873),
    e(6, 910),
    e(6, 948),
];

pub const MCS_TABLE_2: [McsEntry; 28] = [
    e(2, 120),
    e(2, 193),
    e(2, 308),
    e(2, 449),
    e(2, 602),
    e(4, 378),
    e(4, 434),
    e(4, 490),
    e(4, 553),
    e(4, 616),
    e(4, 658),
    e(6, 466),
    e(6, 517),
    e(6, 567),
    e(6, 616),
    e(6, 666),
    e(6, 719),
    e(6, 772),
    e(6, 822),
    e(6, 873),
    e(8, 682),
    e(8, 711),
    e(8, 754),
    e(8, 797),
    e(8, 841),
    e(8, 885),
    e(8, 916),
    e(8, 948),
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
