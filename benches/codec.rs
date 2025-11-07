//! Codec performance benchmarks
//!
//! Measures encode/decode performance to ensure we meet the <1μs target.

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use mxp::{Message, MessageType};

/// Benchmark message encoding
fn bench_encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("encode");

    // Test different payload sizes
    for size in [0, 64, 256, 1024, 4096, 16384] {
        let payload = vec![0u8; size];
        let message = Message::new(MessageType::Call, payload);

        group.throughput(Throughput::Bytes((32 + size + 8) as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &message, |b, msg| {
            b.iter(|| {
                let encoded = black_box(mxp::protocol::encode(msg));
                black_box(encoded);
            });
        });
    }

    group.finish();
}

/// Benchmark message decoding
fn bench_decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("decode");

    // Test different payload sizes
    for size in [0, 64, 256, 1024, 4096, 16384] {
        let payload = vec![0u8; size];
        let message = Message::new(MessageType::Call, payload);
        let encoded = mxp::protocol::encode(&message);
        let bytes = bytes::Bytes::from(encoded);

        group.throughput(Throughput::Bytes(bytes.len() as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &bytes, |b, data| {
            b.iter(|| {
                let decoded = black_box(mxp::protocol::decode(data.clone()).unwrap());
                black_box(decoded);
            });
        });
    }

    group.finish();
}

/// Benchmark full roundtrip (encode + decode)
fn bench_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("roundtrip");

    for size in [0, 64, 256, 1024, 4096] {
        let payload = vec![0u8; size];
        let message = Message::new(MessageType::Call, payload);

        group.throughput(Throughput::Bytes((32 + size + 8) as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &message, |b, msg| {
            b.iter(|| {
                let encoded = black_box(mxp::protocol::encode(msg));
                let bytes = bytes::Bytes::from(encoded);
                let decoded = black_box(mxp::protocol::decode(bytes).unwrap());
                black_box(decoded);
            });
        });
    }

    group.finish();
}

/// Benchmark header encoding
fn bench_header_encode(c: &mut Criterion) {
    use mxp::protocol::MessageHeader;

    c.bench_function("header_encode", |b| {
        let header = MessageHeader::new(MessageType::Call, 123, 456, 789);
        b.iter(|| {
            let bytes = black_box(header.to_bytes());
            black_box(bytes);
        });
    });
}

/// Benchmark header decoding
fn bench_header_decode(c: &mut Criterion) {
    use mxp::protocol::MessageHeader;

    let header = MessageHeader::new(MessageType::Call, 123, 456, 789);
    let bytes = header.to_bytes();

    c.bench_function("header_decode", |b| {
        b.iter(|| {
            let decoded = black_box(MessageHeader::from_bytes(&bytes).unwrap());
            black_box(decoded);
        });
    });
}

/// Benchmark checksum calculation
fn bench_checksum(c: &mut Criterion) {
    use xxhash_rust::xxh3::xxh3_64;

    let mut group = c.benchmark_group("checksum");

    for size in [32, 64, 256, 1024, 4096, 16384] {
        let data = vec![0u8; size];

        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &data, |b, d| {
            b.iter(|| {
                let checksum = black_box(xxh3_64(d));
                black_box(checksum);
            });
        });
    }

    group.finish();
}

/// Benchmark different message types
fn bench_message_types(c: &mut Criterion) {
    let mut group = c.benchmark_group("message_types");

    let types = [
        ("AgentRegister", MessageType::AgentRegister),
        ("AgentDiscover", MessageType::AgentDiscover),
        ("AgentHeartbeat", MessageType::AgentHeartbeat),
        ("Call", MessageType::Call),
        ("Response", MessageType::Response),
        ("Event", MessageType::Event),
        ("StreamOpen", MessageType::StreamOpen),
        ("StreamChunk", MessageType::StreamChunk),
        ("StreamClose", MessageType::StreamClose),
        ("Ack", MessageType::Ack),
        ("Error", MessageType::Error),
    ];

    let payload = vec![0u8; 256];

    for (name, msg_type) in types {
        let message = Message::new(msg_type, payload.clone());

        group.bench_with_input(BenchmarkId::new("encode", name), &message, |b, msg| {
            b.iter(|| {
                let encoded = black_box(mxp::protocol::encode(msg));
                black_box(encoded);
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_encode,
    bench_decode,
    bench_roundtrip,
    bench_header_encode,
    bench_header_decode,
    bench_checksum,
    bench_message_types
);

criterion_main!(benches);
