use nr_core::{Bits, CellId, Point, UeId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UeSlot {
    index: u32,
    generation: u32,
}

impl UeSlot {
    #[inline]
    pub fn index(self) -> usize {
        self.index as usize
    }

    #[inline]
    pub fn generation(self) -> u32 {
        self.generation
    }
}

#[derive(Debug, Clone, Copy)]
pub struct UeState {
    pub id: UeId,
    pub serving_cell: CellId,
    pub position: Point,
    pub backlog: Bits,
}

#[derive(Debug, Default)]
pub struct UeStore {
    ids: Vec<UeId>,
    serving_cells: Vec<CellId>,
    positions: Vec<Point>,
    backlogs: Vec<Bits>,

    generations: Vec<u32>,
    alive: Vec<bool>,
    free_list: Vec<u32>,
    live_count: usize,
}

impl UeStore {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            ids: Vec::with_capacity(capacity),
            serving_cells: Vec::with_capacity(capacity),
            positions: Vec::with_capacity(capacity),
            backlogs: Vec::with_capacity(capacity),
            generations: Vec::with_capacity(capacity),
            alive: Vec::with_capacity(capacity),
            free_list: Vec::new(),
            live_count: 0,
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.live_count
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.live_count == 0
    }

    #[inline]
    pub fn spawn(&mut self, state: UeState) -> UeSlot {
        self.live_count += 1;
        if let Some(index) = self.free_list.pop() {
            let i = index as usize;
            self.ids[i] = state.id;
            self.serving_cells[i] = state.serving_cell;
            self.positions[i] = state.position;
            self.backlogs[i] = state.backlog;
            self.alive[i] = true;
            UeSlot {
                index,
                generation: self.generations[i],
            }
        } else {
            let index = self.ids.len() as u32;
            self.ids.push(state.id);
            self.serving_cells.push(state.serving_cell);
            self.positions.push(state.position);
            self.backlogs.push(state.backlog);
            self.generations.push(0);
            self.alive.push(true);
            UeSlot {
                index,
                generation: 0,
            }
        }
    }

    #[inline]
    pub fn despawn(&mut self, slot: UeSlot) -> bool {
        let i = slot.index();
        if !self.is_current(slot) {
            return false;
        }
        self.alive[i] = false;
        self.generations[i] = self.generations[i].wrapping_add(1);
        self.free_list.push(slot.index);
        self.live_count -= 1;
        true
    }

    #[inline]
    fn is_current(&self, slot: UeSlot) -> bool {
        let i = slot.index();
        i < self.alive.len() && self.alive[i] && self.generations[i] == slot.generation
    }

    #[inline]
    pub fn id_of(&self, slot: UeSlot) -> Option<UeId> {
        self.is_current(slot).then(|| self.ids[slot.index()])
    }

    #[inline]
    pub fn get(&self, slot: UeSlot) -> Option<UeState> {
        if !self.is_current(slot) {
            return None;
        }
        let i = slot.index();
        Some(UeState {
            id: self.ids[i],
            serving_cell: self.serving_cells[i],
            position: self.positions[i],
            backlog: self.backlogs[i],
        })
    }

    #[inline]
    pub fn set_position(&mut self, slot: UeSlot, position: Point) -> bool {
        if !self.is_current(slot) {
            return false;
        }
        self.positions[slot.index()] = position;
        true
    }

    #[inline]
    pub fn set_backlog(&mut self, slot: UeSlot, backlog: Bits) -> bool {
        if !self.is_current(slot) {
            return false;
        }
        self.backlogs[slot.index()] = backlog;
        true
    }

    #[inline]
    pub fn add_backlog(&mut self, slot: UeSlot, delta: Bits) -> bool {
        if !self.is_current(slot) {
            return false;
        }
        let i = slot.index();
        self.backlogs[i] += delta;
        true
    }

    #[inline]
    pub fn iter_slots(&self) -> impl Iterator<Item = UeSlot> + '_ {
        self.alive.iter().enumerate().filter_map(|(i, &alive)| {
            alive.then(|| UeSlot {
                index: i as u32,
                generation: self.generations[i],
            })
        })
    }

    /// SoA 配列の物理長（死スロット含む）。RadioMap の行幅に一致する。
    #[inline]
    pub fn array_len(&self) -> usize {
        self.ids.len()
    }

    /// ids の SoA 生スライス（死スロット含む密配列）。
    /// `ChannelModel::rx_power_batch` へそのまま渡すための借用ビュー。
    #[inline]
    pub fn ids_raw(&self) -> &[UeId] {
        &self.ids
    }

    /// positions の SoA 生スライス（死スロット含む密配列）。
    #[inline]
    pub fn positions_raw(&self) -> &[Point] {
        &self.positions
    }
}
