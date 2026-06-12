use nr_core::Db;

pub fn effective_sinr_average(samples: &[Db]) -> Option<Db> {
    if samples.is_empty() {
        return None;
    }
    let sum = samples.iter().map(|s| s.value()).sum();
    Some(Db::new(sum / samples.len() as f64))
}
