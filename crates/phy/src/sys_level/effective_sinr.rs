use nr_core::Db;
use sap::SinrContext;

pub fn from_context(sinr: &SinrContext) -> Db {
    sinr.sinr_db()
}
