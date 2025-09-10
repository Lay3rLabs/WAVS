mod dev_triggers_benchmark;
mod setup;

use criterion::{criterion_group, criterion_main};

criterion_group!(benches, dev_triggers_benchmark::benchmark);
criterion_main!(benches);