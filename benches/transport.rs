//! Transport layer performance benchmarks
//!
//! Measures transport primitives performance.

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};

/// Benchmark buffer pool operations
fn bench_buffer_pool(c: &mut Criterion) {
    use mxp::transport::buffer::BufferPool;

    let mut group = c.benchmark_group("buffer_pool");

    // Benchmark acquire
    group.bench_function("acquire", |b| {
        let pool = BufferPool::new(2048, 1024);
        b.iter(|| {
            let buffer = black_box(pool.acquire());
            black_box(buffer);
        });
    });

    // Benchmark acquire + release cycle
    group.bench_function("acquire_release", |b| {
        let pool = BufferPool::new(2048, 1024);
        b.iter(|| {
            let buffer = pool.acquire();
            drop(buffer); // Releases back to pool
        });
    });

    group.finish();
}

/// Benchmark congestion control
fn bench_congestion_control(c: &mut Criterion) {
    use mxp::transport::congestion::CongestionController;
    use std::time::{Duration, Instant};

    let mut group = c.benchmark_group("congestion_control");

    group.bench_function("on_ack", |b| {
        let mut controller = CongestionController::new();
        let now = Instant::now();
        let rtt = Duration::from_millis(10);

        b.iter(|| {
            controller.on_ack(black_box(1024), black_box(now), black_box(rtt));
        });
    });

    group.bench_function("on_loss", |b| {
        let mut controller = CongestionController::new();

        b.iter(|| {
            controller.on_loss(black_box(1024));
        });
    });

    group.finish();
}

/// Benchmark flow control
fn bench_flow_control(c: &mut Criterion) {
    use mxp::transport::flow::FlowController;

    let mut group = c.benchmark_group("flow_control");

    group.bench_function("consume", |b| {
        let mut controller = FlowController::new(1_000_000, 100_000);

        b.iter(|| {
            let _ = controller.consume(black_box(1024));
        });
    });

    group.bench_function("release", |b| {
        let mut controller = FlowController::new(1_000_000, 100_000);

        b.iter(|| {
            controller.release(black_box(1024));
        });
    });

    group.finish();
}

/// Benchmark ACK frame operations
fn bench_ack_frame(c: &mut Criterion) {
    use mxp::transport::ack::{AckFrame, AckRange};

    let mut group = c.benchmark_group("ack_frame");

    // Benchmark encoding
    group.bench_function("encode", |b| {
        let ranges = vec![
            AckRange::new(100, 110).unwrap(),
            AckRange::new(80, 90).unwrap(),
            AckRange::new(50, 60).unwrap(),
        ];
        let frame = AckFrame::new(110, ranges);

        b.iter(|| {
            let encoded = black_box(frame.encode());
            black_box(encoded);
        });
    });

    // Benchmark decoding
    group.bench_function("decode", |b| {
        let ranges = vec![
            AckRange::new(100, 110).unwrap(),
            AckRange::new(80, 90).unwrap(),
            AckRange::new(50, 60).unwrap(),
        ];
        let frame = AckFrame::new(110, ranges);
        let encoded = frame.encode();

        b.iter(|| {
            let decoded = black_box(AckFrame::decode(&encoded).unwrap());
            black_box(decoded);
        });
    });

    group.finish();
}

/// Benchmark crypto operations
fn bench_crypto(c: &mut Criterion) {
    use mxp::transport::crypto::{chacha20_poly1305_open, chacha20_poly1305_seal};

    let mut group = c.benchmark_group("crypto");

    for size in [64, 256, 1024, 4096] {
        let key = [0u8; 32];
        let nonce = [0u8; 12];
        let plaintext = vec![0u8; size];
        let aad = b"additional data";

        group.throughput(Throughput::Bytes(size as u64));

        // Benchmark seal (encrypt)
        group.bench_with_input(BenchmarkId::new("seal", size), &plaintext, |b, data| {
            b.iter(|| {
                let ciphertext = black_box(chacha20_poly1305_seal(&key, &nonce, data, aad));
                black_box(ciphertext);
            });
        });

        // Benchmark open (decrypt)
        let ciphertext = chacha20_poly1305_seal(&key, &nonce, &plaintext, aad);
        group.bench_with_input(BenchmarkId::new("open", size), &ciphertext, |b, data| {
            b.iter(|| {
                let plaintext = black_box(chacha20_poly1305_open(&key, &nonce, data, aad).unwrap());
                black_box(plaintext);
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_buffer_pool,
    bench_congestion_control,
    bench_flow_control,
    bench_ack_frame,
    bench_crypto
);

criterion_main!(benches);
