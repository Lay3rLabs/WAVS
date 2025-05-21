mod block_intervals;
mod handle;

use criterion::{criterion_group, criterion_main};

criterion_group!(benches, block_intervals::benchmark);
criterion_main!(benches);
