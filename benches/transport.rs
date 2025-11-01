// Transport benchmarks placeholder
// Will be implemented with actual QUIC transport tests

use criterion::{criterion_group, criterion_main, Criterion};

fn bench_placeholder(c: &mut Criterion) {
    c.bench_function("transport_placeholder", |b| {
        b.iter(|| {
            // Placeholder
        });
    });
}

criterion_group!(benches, bench_placeholder);
criterion_main!(benches);
