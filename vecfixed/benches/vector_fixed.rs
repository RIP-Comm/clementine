use criterion::{criterion_group, criterion_main, Criterion};
use vecfixed::VecFixed;

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("vector_fixed", |b| {
        let mut v = VecFixed::<100, u32>::new();
        b.iter(|| {
            for i in 0..1000 {
                v.push(i);
            }

            v.join(" ")
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
