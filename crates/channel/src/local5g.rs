use nr_core::{CellId, Db, Hz, Point, SimRng, UeId, Watt};
use sap::{ChannelModel, SlotContext};

const D_NEAR_KM: f64 = 0.04;
const D_FAR_KM: f64 = 0.10;

const MIN_DISTANCE_KM: f64 = 1.0e-3;

const ALPHA_PIVOT_KM: f64 = 20.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AreaType {
    Urban,
    Suburban,
    Open,
}

impl AreaType {
    #[inline]
    fn s_db(self) -> f64 {
        match self {
            AreaType::Urban => 0.0,
            AreaType::Suburban => 12.3,
            AreaType::Open => 32.5,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CitySize {
    Large,
    Medium,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Local5gParams {
    pub area: AreaType,
    pub city: CitySize,
    pub r_db: f64,
    pub k_db: f64,
}

impl Default for Local5gParams {
    fn default() -> Self {
        Self {
            area: AreaType::Urban,
            city: CitySize::Medium,
            r_db: 0.0,
            k_db: 0.0,
        }
    }
}

pub struct Local5gChannel {
    f_mhz: f64,
    params: Local5gParams,
}

impl Local5gChannel {
    pub fn new(fc: Hz, params: Local5gParams) -> Self {
        Self {
            f_mhz: fc.to_mhz(),
            params,
        }
    }

    pub fn with_defaults(fc: Hz) -> Self {
        Self::new(fc, Local5gParams::default())
    }

    #[inline]
    fn a_hm(&self, hm: f64) -> f64 {
        let log_f = self.f_mhz.log10();
        match self.params.city {
            CitySize::Medium => (1.1 * log_f - 0.7) * hm - (1.56 * log_f - 0.8),
            CitySize::Large => 3.2 * (11.75 * hm).log10().powi(2) - 4.97,
        }
    }

    #[inline]
    fn b_hb(hb: f64) -> f64 {
        if hb >= 30.0 {
            0.0
        } else {
            20.0 * (hb / 30.0).log10()
        }
    }

    #[inline]
    fn alpha(&self, d_km: f64, hb: f64) -> f64 {
        if d_km <= ALPHA_PIVOT_KM {
            1.0
        } else {
            let coeff = 0.14 + 1.87e-4 * self.f_mhz + 1.07e-3 * hb;
            let term = (d_km / ALPHA_PIVOT_KM).log10().powf(0.8);
            1.0 / (1.0 + coeff * term)
        }
    }

    #[inline]
    fn l0(&self, d_km: f64, hb: f64, hm: f64) -> f64 {
        let dh = hb - hm;
        let inner = d_km * d_km + (dh * dh) / 1.0e6;
        32.4 + 20.0 * self.f_mhz.log10() + 10.0 * inner.log10() + self.params.r_db
    }

    #[inline]
    fn lh(&self, d_km: f64, hb: f64, hm: f64) -> f64 {
        let hb_eff = hb.max(30.0);
        let log_hb = hb_eff.log10();
        let alpha = self.alpha(d_km, hb);

        46.3 + 33.9 * 2000.0_f64.log10() + 10.0 * (self.f_mhz / 2000.0).log10() - 13.82 * log_hb
            + (44.9 - 6.55 * log_hb) * d_km.log10() * alpha
            - self.a_hm(hm)
            - Self::b_hb(hb)
            + self.params.r_db
            - self.params.k_db
            - self.params.area.s_db()
    }

    fn pathloss_db(&self, d_km: f64, hb: f64, hm: f64) -> f64 {
        let d_km = d_km.max(MIN_DISTANCE_KM);
        let l0 = self.l0(d_km, hb, hm);

        let l = if d_km <= D_NEAR_KM {
            l0
        } else if d_km < D_FAR_KM {
            let lh = self.lh(d_km, hb, hm);
            let w = 2.51 * d_km.log10() + 3.51;
            l0 + w * (lh - l0)
        } else {
            self.lh(d_km, hb, hm)
        };

        l.max(l0)
    }
}

impl ChannelModel for Local5gChannel {
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
        let d_km = tx_pos.distance_2d(&rx_pos).value() / 1000.0;
        let hb = tx_pos.height();
        let hm = rx_pos.height();

        let pl_db = self.pathloss_db(d_km, hb, hm);
        let rx_dbm = tx_power.to_dbm() - Db::new(pl_db);
        rx_dbm.to_watt()
    }
}
