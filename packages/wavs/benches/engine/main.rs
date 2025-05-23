mod engine_execute;
mod handle;

use criterion::{criterion_group, criterion_main};

criterion_group!(benches, engine_execute::benchmark);
criterion_main!(benches);