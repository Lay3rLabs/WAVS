mod engine_system;
mod setup;

use criterion::{criterion_group, criterion_main};

criterion_group!(benches, engine_system::benchmark);
criterion_main!(benches);
