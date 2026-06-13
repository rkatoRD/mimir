use nr_core::Watt;
use sap::ChannelModel;

use crate::{cell_store::CellStore, ue_store::UeStore};

pub(crate) struct RadioMap {
    rx_w: Vec<f64>,
    total_rx_w: Vec<f64>,
    row_buf: Vec<Watt>,
    ue_cap: usize,
    dirty: bool,
}

impl RadioMap {
    pub(crate) fn with_capacity(n_cells: usize, ue_cap: usize) -> Self {
        Self {
            rx_w: Vec::with_capacity(n_cells * ue_cap),
            total_rx_w: Vec::with_capacity(ue_cap),
            row_buf: Vec::with_capacity(ue_cap),
            ue_cap,
            dirty: true,
        }
    }

    #[inline]
    pub(crate) fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    #[inline]
    pub(crate) fn is_dirty(&self) -> bool {
        self.dirty
    }

    #[inline]
    pub(crate) fn rx(&self, cell_index: usize, ue_index: usize) -> f64 {
        self.rx_w[cell_index * self.ue_cap + ue_index]
    }

    #[inline]
    pub(crate) fn total(&self, ue_index: usize) -> f64 {
        self.total_rx_w[ue_index]
    }

    pub(crate) fn rebuild<CH: ChannelModel>(
        &mut self,
        channel: &CH,
        cells: &CellStore,
        ues: &UeStore,
    ) {
        let n_cells = cells.len();
        let ue_cap = ues.array_len();
        self.ue_cap = ue_cap;

        self.rx_w.clear();
        self.rx_w.resize(n_cells * ue_cap, 0.0);
        self.total_rx_w.clear();
        self.total_rx_w.resize(ue_cap, 0.0);
        self.row_buf.clear();
        self.row_buf.resize(ue_cap, Watt::new(0.0));

        let (cell_ids, cell_pos, cell_pw) = cells.geometry();
        let ue_ids = ues.ids_raw();
        let ue_pos = ues.positions_raw();

        for c in 0..n_cells {
            channel.rx_power_batch(
                cell_ids[c],
                cell_pw[c],
                cell_pos[c],
                ue_ids,
                ue_pos,
                &mut self.row_buf,
            );
            let row = &mut self.rx_w[c * ue_cap..(c + 1) * ue_cap];
            for u in 0..ue_cap {
                let w = self.row_buf[u].value();
                row[u] = w;
                self.total_rx_w[u] += w;
            }
        }

        self.dirty = false;
    }
}
