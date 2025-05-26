mod block_intervals;
mod setup;

use criterion::{criterion_group, criterion_main};

criterion_group!(benches, block_intervals::benchmark);
criterion_main!(benches);
