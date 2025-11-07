//! Comparison benchmarks: MXP vs HTTP vs gRPC
//!
//! This benchmark compares MXP codec performance against HTTP and simulated gRPC
//! to validate performance claims.

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use mxp::{Message, MessageType};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Benchmark MXP codec (baseline)
fn bench_mxp_codec(c: &mut Criterion) {
    let mut group = c.benchmark_group("mxp_codec");

    for size in [64, 256, 1024] {
        let payload = vec![0u8; size];
        let message = Message::new(MessageType::Call, payload);

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

/// Benchmark JSON serialization (HTTP-like)
fn bench_json_serde(c: &mut Criterion) {
    #[derive(Serialize, Deserialize)]
    struct JsonMessage {
        msg_type: u8,
        message_id: u64,
        trace_id: u64,
        payload: Vec<u8>,
    }

    let mut group = c.benchmark_group("json_serde");

    for size in [64, 256, 1024] {
        let msg = JsonMessage {
            msg_type: 0x10,
            message_id: 12345,
            trace_id: 67890,
            payload: vec![0u8; size],
        };

        group.bench_with_input(BenchmarkId::from_parameter(size), &msg, |b, m| {
            b.iter(|| {
                let json = black_box(serde_json::to_vec(m).unwrap());
                let decoded: JsonMessage = black_box(serde_json::from_slice(&json).unwrap());
                black_box(decoded);
            });
        });
    }

    group.finish();
}

/// Benchmark Protocol Buffers-like encoding (simulated)
fn bench_protobuf_like(c: &mut Criterion) {
    // Simulate protobuf with bincode (similar binary format)
    #[derive(Serialize, Deserialize)]
    struct ProtoMessage {
        msg_type: u8,
        message_id: u64,
        trace_id: u64,
        payload: Vec<u8>,
    }

    let mut group = c.benchmark_group("protobuf_like");

    for size in [64, 256, 1024] {
        let msg = ProtoMessage {
            msg_type: 0x10,
            message_id: 12345,
            trace_id: 67890,
            payload: vec![0u8; size],
        };

        group.bench_with_input(BenchmarkId::from_parameter(size), &msg, |b, m| {
            b.iter(|| {
                // Simulate protobuf encoding with bincode
                let encoded = black_box(bincode::serialize(m).unwrap());
                let decoded: ProtoMessage = black_box(bincode::deserialize(&encoded).unwrap());
                black_box(decoded);
            });
        });
    }

    group.finish();
}

/// Benchmark MessagePack encoding
fn bench_messagepack(c: &mut Criterion) {
    #[derive(Serialize, Deserialize)]
    struct MsgPackMessage {
        msg_type: u8,
        message_id: u64,
        trace_id: u64,
        payload: Vec<u8>,
    }

    let mut group = c.benchmark_group("messagepack");

    for size in [64, 256, 1024] {
        let msg = MsgPackMessage {
            msg_type: 0x10,
            message_id: 12345,
            trace_id: 67890,
            payload: vec![0u8; size],
        };

        group.bench_with_input(BenchmarkId::from_parameter(size), &msg, |b, m| {
            b.iter(|| {
                let encoded = black_box(rmp_serde::to_vec(m).unwrap());
                let decoded: MsgPackMessage = black_box(rmp_serde::from_slice(&encoded).unwrap());
                black_box(decoded);
            });
        });
    }

    group.finish();
}

/// Comparison: All protocols side-by-side
fn bench_all_protocols(c: &mut Criterion) {
    #[derive(Serialize, Deserialize, Clone)]
    struct GenericMessage {
        msg_type: u8,
        message_id: u64,
        trace_id: u64,
        payload: Vec<u8>,
    }

    let mut group = c.benchmark_group("comparison_256B");
    group.measurement_time(Duration::from_secs(10)); // Longer for accuracy

    let payload = vec![0u8; 256];

    // MXP
    let mxp_msg = Message::new(MessageType::Call, payload.clone());
    group.bench_function("MXP", |b| {
        b.iter(|| {
            let encoded = black_box(mxp::protocol::encode(&mxp_msg));
            let bytes = bytes::Bytes::from(encoded);
            let decoded = black_box(mxp::protocol::decode(bytes).unwrap());
            black_box(decoded);
        });
    });

    // JSON
    let json_msg = GenericMessage {
        msg_type: 0x10,
        message_id: 12345,
        trace_id: 67890,
        payload: payload.clone(),
    };
    group.bench_function("JSON", |b| {
        b.iter(|| {
            let json = black_box(serde_json::to_vec(&json_msg).unwrap());
            let decoded: GenericMessage = black_box(serde_json::from_slice(&json).unwrap());
            black_box(decoded);
        });
    });

    // Bincode (protobuf-like)
    group.bench_function("Bincode", |b| {
        b.iter(|| {
            let encoded = black_box(bincode::serialize(&json_msg).unwrap());
            let decoded: GenericMessage = black_box(bincode::deserialize(&encoded).unwrap());
            black_box(decoded);
        });
    });

    // MessagePack
    group.bench_function("MessagePack", |b| {
        b.iter(|| {
            let encoded = black_box(rmp_serde::to_vec(&json_msg).unwrap());
            let decoded: GenericMessage = black_box(rmp_serde::from_slice(&encoded).unwrap());
            black_box(decoded);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_mxp_codec,
    bench_json_serde,
    bench_protobuf_like,
    bench_messagepack,
    bench_all_protocols
);

criterion_main!(benches);
