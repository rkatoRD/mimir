use nr_spec::{McsEntry, McsTable};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct McsConfig {
    pub table: McsTable,
}

impl McsConfig {
    pub const fn new(table: McsTable) -> Self {
        Self { table }
    }

    pub fn entry(&self, mcs_index: u8) -> Option<McsEntry> {
        self.table.entry(mcs_index)
    }
}

impl Default for McsConfig {
    fn default() -> Self {
        Self::new(McsTable::Table1)
    }
}

pub fn spectral_efficiency(table: McsTable, mcs_index: u8) -> f64 {
    table
        .entry(mcs_index)
        .map(|e| e.spectral_efficiency())
        .unwrap_or(0.0)
}
