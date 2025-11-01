// Transport benchmarks placeholder
// Will be implemented with actual QUIC transport tests

use criterion::{Criterion, criterion_group, criterion_main};

fn bench_placeholder(c: &mut Criterion) {
    c.bench_function("transport_placeholder", |b| {
        b.iter(|| {
            // Placeholder
        });
    });
}

criterion_group!(benches, bench_placeholder);
criterion_main!(benches);
