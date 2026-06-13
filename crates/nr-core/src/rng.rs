use rand::SeedableRng;
use rand_chacha::ChaChaRng;

use crate::config::SimConfig;

pub struct SimRng(ChaChaRng);

impl SimRng {
    pub fn from_seed(seed: u64) -> Self {
        Self(ChaChaRng::seed_from_u64(seed))
    }

    pub fn from_config(config: &SimConfig) -> Self {
        Self::from_seed(config.seed)
    }

    #[inline]
    pub fn inner(&mut self) -> &mut ChaChaRng {
        &mut self.0
    }
}
