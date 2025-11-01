use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use mxp::{Message, MessageType};

fn bench_encode(c: &mut Criterion) {
    let mut group = c.benchmark_group("codec");

    // Small message (64 bytes)
    let small_msg = Message::new(MessageType::Call, vec![0u8; 64]);
    group.throughput(Throughput::Bytes(64));
    group.bench_function("encode_64b", |b| {
        b.iter(|| {
            black_box(small_msg.encode());
        });
    });

    // Medium message (1 KB)
    let medium_msg = Message::new(MessageType::Call, vec![0u8; 1024]);
    group.throughput(Throughput::Bytes(1024));
    group.bench_function("encode_1kb", |b| {
        b.iter(|| {
            black_box(medium_msg.encode());
        });
    });

    // Large message (64 KB)
    let large_msg = Message::new(MessageType::Call, vec![0u8; 64 * 1024]);
    group.throughput(Throughput::Bytes(64 * 1024));
    group.bench_function("encode_64kb", |b| {
        b.iter(|| {
            black_box(large_msg.encode());
        });
    });

    group.finish();
}

fn bench_decode(c: &mut Criterion) {
    let mut group = c.benchmark_group("codec");

    // Small message (64 bytes)
    let small_msg = Message::new(MessageType::Call, vec![0u8; 64]);
    let small_encoded = small_msg.encode();
    group.throughput(Throughput::Bytes(64));
    group.bench_function("decode_64b", |b| {
        b.iter(|| {
            black_box(Message::decode(&small_encoded).unwrap());
        });
    });

    // Medium message (1 KB)
    let medium_msg = Message::new(MessageType::Call, vec![0u8; 1024]);
    let medium_encoded = medium_msg.encode();
    group.throughput(Throughput::Bytes(1024));
    group.bench_function("decode_1kb", |b| {
        b.iter(|| {
            black_box(Message::decode(&medium_encoded).unwrap());
        });
    });

    // Large message (64 KB)
    let large_msg = Message::new(MessageType::Call, vec![0u8; 64 * 1024]);
    let large_encoded = large_msg.encode();
    group.throughput(Throughput::Bytes(64 * 1024));
    group.bench_function("decode_64kb", |b| {
        b.iter(|| {
            black_box(Message::decode(&large_encoded).unwrap());
        });
    });

    group.finish();
}

fn bench_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("codec");

    let msg = Message::new(MessageType::Call, vec![0u8; 1024]);
    group.throughput(Throughput::Bytes(1024));
    group.bench_function("roundtrip_1kb", |b| {
        b.iter(|| {
            let encoded = msg.encode();
            black_box(Message::decode(&encoded).unwrap());
        });
    });

    group.finish();
}

criterion_group!(benches, bench_encode, bench_decode, bench_roundtrip);
criterion_main!(benches);
