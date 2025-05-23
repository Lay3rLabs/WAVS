mod engine_execute;

use criterion::{criterion_group, criterion_main};

criterion_group!(benches, engine_execute::benchmark);
criterion_main!(benches);