use nr_core::{CellId, Db, Hz, Point, SimRng, UeId, Watt};
use sap::{ChannelModel, SlotContext};

const SPEED_OF_LIGHT: f64 = 299_792_458.0;

const MIN_DISTANCE_M: f64 = 1.0;

pub struct FreeSpaceChannel {
    const_offset_db: f64,
}

impl FreeSpaceChannel {
    pub fn new(fc: Hz) -> Hz {
        let f = fc.value();
        let const_offset_db =
            20.0 * f.log10() + 20.0 * (4.0 * std::f64::consts::PI / SPEED_OF_LIGHT).log10();
        Self { const_offset_db }
    }

    #[inline]
    fn pathloss_db(&self, distance_m: f64) -> f64 {
        let d = distance_m.max(MIN_DISTANCE_M);
        20.0 * d.log10() + self.const_offset_db
    }
}

impl ChannelModel for FreeSpaceChannel {
    fn update(&mut self, _ctx: &SlotContext, _rng: &mut SimRng) -> bool {
        false
    }

    fn rx_power(
        &self,
        _from: CellId,
        _to: UeId,
        tx_power: Watt,
        tx_pos: Point,
        rx_pos: Point,
    ) -> Watt {
        let distance = tx_pos.distance_3d(&rx_pos).value();
        let pl_db = self.pathloss_db(distance);
        let rx_dbm = tx_power.to_dbm() - Db::new(pl_db);
        rx_dbm.to_watt()
    }
}
