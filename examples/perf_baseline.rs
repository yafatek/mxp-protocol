//! MXP transport performance smoke benchmarks.
//!
//! Run with `cargo run --example perf_baseline --release` (optionally set
//! `MXP_BENCH_ITERS` to control the iteration count).

use std::env;
use std::time::{Duration, Instant};

use mxp::transport::AeadKey;
use mxp::transport::{
    AmplificationConfig, AntiAmplificationGuard, BufferPool, DatagramConfig, DatagramQueue,
    EndpointRole, HeaderProtectionKey, PacketCipher, PacketFlags, PriorityClass, Scheduler,
    SessionKeys, StreamId, StreamKind, StreamManager,
};

const DEFAULT_ITERATIONS: usize = 100_000;

fn main() {
    let iterations = iterations_from_env();
    println!("MXP perf baseline — iterations: {iterations}");
    println!("-----------------------------------------------------------------");

    bench_packet_path(iterations);
    bench_stream_cycle(iterations);
    bench_scheduler(iterations);
    bench_datagram_queue(iterations);
}

fn iterations_from_env() -> usize {
    env::var("MXP_BENCH_ITERS")
        .ok()
        .and_then(|raw| raw.parse::<usize>().ok())
        .filter(|iters| *iters > 0)
        .unwrap_or(DEFAULT_ITERATIONS)
}

fn run_bench<F>(label: &str, iterations: usize, mut body: F)
where
    F: FnMut(),
{
    const WARMUP: usize = 1_000;
    for _ in 0..WARMUP {
        body();
    }

    let start = Instant::now();
    for _ in 0..iterations {
        body();
    }
    let elapsed = start.elapsed();

    report(label, iterations, elapsed);
}

fn report(label: &str, iterations: usize, elapsed: Duration) {
    let secs = elapsed.as_secs_f64().max(f64::MIN_POSITIVE);
    let ns_per_op = elapsed.as_nanos() as f64 / iterations as f64;
    let throughput = iterations as f64 / secs;
    let mops = throughput / 1_000_000.0;
    println!("{label:<32} total={elapsed:?} | {ns_per_op:>8.1} ns/op | {mops:>7.2} Mops/s");
}

fn bench_packet_path(iterations: usize) {
    let pool = BufferPool::new(2048, 2);
    let mut buffer = pool.acquire();
    let payload = vec![0u8; 512];

    let initiator_keys = SessionKeys::new(
        AeadKey::from_array([0x11; mxp::transport::AEAD_KEY_LEN]),
        AeadKey::from_array([0x22; mxp::transport::AEAD_KEY_LEN]),
        HeaderProtectionKey::from_array([0x33; mxp::transport::HEADER_PROTECTION_KEY_LEN]),
        HeaderProtectionKey::from_array([0x44; mxp::transport::HEADER_PROTECTION_KEY_LEN]),
    );
    let responder_keys = SessionKeys::new(
        AeadKey::from_array([0x22; mxp::transport::AEAD_KEY_LEN]),
        AeadKey::from_array([0x11; mxp::transport::AEAD_KEY_LEN]),
        HeaderProtectionKey::from_array([0x44; mxp::transport::HEADER_PROTECTION_KEY_LEN]),
        HeaderProtectionKey::from_array([0x33; mxp::transport::HEADER_PROTECTION_KEY_LEN]),
    );

    let mut sender = PacketCipher::new(initiator_keys);
    let mut receiver = PacketCipher::new(responder_keys);

    run_bench("packet_seal+open", iterations, || {
        buffer.reset();
        let (_, written) = sender
            .seal_into(
                0x4D58_5031,
                PacketFlags::default(),
                &payload,
                buffer.as_mut_slice(),
            )
            .expect("seal into buffer");
        buffer.set_len(written);
        receiver.open(buffer.as_slice()).expect("decrypt packet");
    });
}

fn bench_stream_cycle(iterations: usize) {
    let mut manager = StreamManager::new(EndpointRole::Client);
    let stream_id = StreamId::new(EndpointRole::Client, StreamKind::Bidirectional, 0);
    manager.get_or_create(stream_id);
    manager.set_connection_limit(u64::MAX / 8);
    manager.set_stream_limit(stream_id, u64::MAX / 8);
    let payload = vec![0u8; 256];

    run_bench("stream_send_ingest", iterations, || {
        manager.queue_send(stream_id, &payload).expect("queue send");
        let chunk = manager
            .poll_send_chunk(stream_id, payload.len())
            .expect("flow ok")
            .expect("chunk available");
        manager
            .ingest(stream_id, chunk.offset, &chunk.payload, chunk.fin)
            .expect("ingest");
        let received = manager.read(stream_id, payload.len()).expect("read");
        debug_assert_eq!(received.len(), payload.len());
    });
}

fn bench_scheduler(iterations: usize) {
    let mut scheduler = Scheduler::new();
    let stream_hi = StreamId::new(EndpointRole::Client, StreamKind::Bidirectional, 1);
    let stream_lo = StreamId::new(EndpointRole::Client, StreamKind::Bidirectional, 2);

    run_bench("scheduler_push_pop", iterations, || {
        scheduler.push_stream(stream_hi, PriorityClass::Control);
        scheduler.push_stream(stream_lo, PriorityClass::Bulk);
        scheduler.pop_stream();
        scheduler.pop_stream();
    });
}

fn bench_datagram_queue(iterations: usize) {
    let mut queue = DatagramQueue::new(DatagramConfig {
        max_payload: 1024,
        max_queue: 64,
    });
    let mut guard = AntiAmplificationGuard::new(AmplificationConfig::default());

    run_bench("datagram_enqueue", iterations, || {
        let payload = vec![0u8; 256];
        queue.enqueue(payload).expect("enqueue");
        guard.on_receive(256);
        let popped = queue.dequeue_with_guard(&mut guard);
        debug_assert!(popped.is_some());
    });
}
