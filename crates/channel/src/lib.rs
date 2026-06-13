#[cfg(feature = "pathloss-freespace")]
pub mod freespace;

#[cfg(feature = "pathloss-local5g")]
pub mod local5g;

#[cfg(feature = "pathloss-inf")]
pub mod inf;

#[cfg(feature = "pathloss-inf")]
pub mod shadowing;

#[cfg(feature = "pathloss-freespace")]
pub use freespace::FreeSpaceChannel;

#[cfg(feature = "pathloss-local5g")]
pub use local5g::{AreaType, CitySize, Local5gChannel, Local5gParams};

#[cfg(feature = "pathloss-inf")]
pub use inf::{InfChannel, InfSubScenario};
