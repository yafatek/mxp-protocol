//! Transport layer performance benchmarks
//!
//! NOTE: Currently disabled as transport APIs are not fully public yet.
//! Will be enabled in a future release when transport modules are stabilized.

use criterion::{Criterion, criterion_group, criterion_main};

/// Placeholder benchmark - transport benchmarks coming soon
fn bench_placeholder(c: &mut Criterion) {
    c.bench_function("transport_placeholder", |b| {
        b.iter(|| {
            // Placeholder - will add real transport benchmarks when APIs are public
            1 + 1
        });
    });
}

criterion_group!(benches, bench_placeholder);
criterion_main!(benches);
