use criterion::{criterion_group, criterion_main, Criterion};

pub fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("mock_bench", |b| {
        //setup();

        b.iter(|| {
            // Code to benchmark
        });

        //teardown();
    });
}

// fn setup() -> InputType {
//     // Your setup code here
//     InputType::new()
// }

// fn teardown(input: InputType) {
//     // Your teardown code here
//     drop(input);
// }

// fn runner(input: &InputType) {
//     // The code you want to benchmark
// }

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
