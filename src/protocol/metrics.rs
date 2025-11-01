#![allow(dead_code)] // Metrics wiring arrives in Phase 4; silence interim warnings.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use super::MessageType;

/// Track MXP protocol metrics without external dependencies.
pub(crate) struct Metrics;

static TOTAL_MESSAGES: AtomicU64 = AtomicU64::new(0);
static SENT_MESSAGES: AtomicU64 = AtomicU64::new(0);
static RECEIVED_MESSAGES: AtomicU64 = AtomicU64::new(0);
static ERROR_COUNT: AtomicU64 = AtomicU64::new(0);
static ACTIVE_CONNECTIONS: AtomicU64 = AtomicU64::new(0);
static ACTIVE_STREAMS: AtomicU64 = AtomicU64::new(0);
static SEND_LATENCY_TOTAL_NS: AtomicU64 = AtomicU64::new(0);
static SEND_LATENCY_MAX_NS: AtomicU64 = AtomicU64::new(0);
static RECV_LATENCY_TOTAL_NS: AtomicU64 = AtomicU64::new(0);
static RECV_LATENCY_MAX_NS: AtomicU64 = AtomicU64::new(0);

static DATAGRAM_ENQUEUED: AtomicU64 = AtomicU64::new(0);
static DATAGRAM_ENQUEUED_BYTES: AtomicU64 = AtomicU64::new(0);
static DATAGRAM_SENT: AtomicU64 = AtomicU64::new(0);
static DATAGRAM_SENT_BYTES: AtomicU64 = AtomicU64::new(0);

static FLOW_BYTES_CONSUMED: AtomicU64 = AtomicU64::new(0);
static FLOW_CONNECTION_UPDATES: AtomicU64 = AtomicU64::new(0);
static FLOW_STREAM_UPDATES: AtomicU64 = AtomicU64::new(0);

static SCHEDULER_CONTROL_ENQUEUED: AtomicU64 = AtomicU64::new(0);
static SCHEDULER_CONTROL_DEQUEUED: AtomicU64 = AtomicU64::new(0);
static SCHEDULER_INTERACTIVE_ENQUEUED: AtomicU64 = AtomicU64::new(0);
static SCHEDULER_INTERACTIVE_DEQUEUED: AtomicU64 = AtomicU64::new(0);
static SCHEDULER_BULK_ENQUEUED: AtomicU64 = AtomicU64::new(0);
static SCHEDULER_BULK_DEQUEUED: AtomicU64 = AtomicU64::new(0);

const NANOSECONDS_PER_MICROSECOND: u128 = 1_000;

struct MessageTypeCounters {
    agent_register: AtomicU64,
    agent_discover: AtomicU64,
    agent_heartbeat: AtomicU64,
    call: AtomicU64,
    response: AtomicU64,
    event: AtomicU64,
    stream_open: AtomicU64,
    stream_chunk: AtomicU64,
    stream_close: AtomicU64,
    ack: AtomicU64,
    error: AtomicU64,
}

static MESSAGE_COUNTERS: MessageTypeCounters = MessageTypeCounters::new();

impl MessageTypeCounters {
    const fn new() -> Self {
        Self {
            agent_register: AtomicU64::new(0),
            agent_discover: AtomicU64::new(0),
            agent_heartbeat: AtomicU64::new(0),
            call: AtomicU64::new(0),
            response: AtomicU64::new(0),
            event: AtomicU64::new(0),
            stream_open: AtomicU64::new(0),
            stream_chunk: AtomicU64::new(0),
            stream_close: AtomicU64::new(0),
            ack: AtomicU64::new(0),
            error: AtomicU64::new(0),
        }
    }

    fn increment(&self, msg_type: MessageType) {
        use MessageType::*;

        match msg_type {
            AgentRegister => self.agent_register.fetch_add(1, Ordering::Relaxed),
            AgentDiscover => self.agent_discover.fetch_add(1, Ordering::Relaxed),
            AgentHeartbeat => self.agent_heartbeat.fetch_add(1, Ordering::Relaxed),
            Call => self.call.fetch_add(1, Ordering::Relaxed),
            Response => self.response.fetch_add(1, Ordering::Relaxed),
            Event => self.event.fetch_add(1, Ordering::Relaxed),
            StreamOpen => self.stream_open.fetch_add(1, Ordering::Relaxed),
            StreamChunk => self.stream_chunk.fetch_add(1, Ordering::Relaxed),
            StreamClose => self.stream_close.fetch_add(1, Ordering::Relaxed),
            Ack => self.ack.fetch_add(1, Ordering::Relaxed),
            MessageType::Error => self.error.fetch_add(1, Ordering::Relaxed),
        };
    }
}

/// Direction of observed latency measurement.
#[derive(Clone, Copy)]
pub(crate) enum LatencyKind {
    Send,
    Receive,
}

/// Direction of message flow for counting.
#[derive(Clone, Copy)]
pub(crate) enum MessageDirection {
    Sent,
    Received,
}

/// Scheduler priority classes for metrics.
#[derive(Clone, Copy)]
pub enum SchedulerPriority {
    Control,
    Interactive,
    Bulk,
}

impl Metrics {
    #[inline]
    pub(crate) fn record_message(direction: MessageDirection, msg_type: MessageType) {
        TOTAL_MESSAGES.fetch_add(1, Ordering::Relaxed);
        match direction {
            MessageDirection::Sent => {
                SENT_MESSAGES.fetch_add(1, Ordering::Relaxed);
            }
            MessageDirection::Received => {
                RECEIVED_MESSAGES.fetch_add(1, Ordering::Relaxed);
            }
        }
        MESSAGE_COUNTERS.increment(msg_type);
    }

    #[inline]
    pub(crate) fn record_error() {
        ERROR_COUNT.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub(crate) fn record_connection_open() {
        ACTIVE_CONNECTIONS.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub(crate) fn record_connection_close() {
        ACTIVE_CONNECTIONS.fetch_sub(1, Ordering::Relaxed);
    }

    #[inline]
    pub(crate) fn record_stream_open() {
        ACTIVE_STREAMS.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub(crate) fn record_stream_close() {
        ACTIVE_STREAMS.fetch_sub(1, Ordering::Relaxed);
    }

    #[inline]
    pub(crate) fn record_latency(kind: LatencyKind, duration: Duration) {
        let nanos = duration
            .as_nanos()
            .min(u64::MAX as u128)
            .try_into()
            .unwrap_or(u64::MAX);

        match kind {
            LatencyKind::Send => {
                SEND_LATENCY_TOTAL_NS.fetch_add(nanos, Ordering::Relaxed);
                update_max(&SEND_LATENCY_MAX_NS, nanos);
            }
            LatencyKind::Receive => {
                RECV_LATENCY_TOTAL_NS.fetch_add(nanos, Ordering::Relaxed);
                update_max(&RECV_LATENCY_MAX_NS, nanos);
            }
        }
    }

    #[inline]
    pub(crate) fn record_datagram_enqueued(len: usize) {
        DATAGRAM_ENQUEUED.fetch_add(1, Ordering::Relaxed);
        DATAGRAM_ENQUEUED_BYTES.fetch_add(len as u64, Ordering::Relaxed);
    }

    #[inline]
    pub(crate) fn record_datagram_sent(len: usize) {
        DATAGRAM_SENT.fetch_add(1, Ordering::Relaxed);
        DATAGRAM_SENT_BYTES.fetch_add(len as u64, Ordering::Relaxed);
    }

    #[inline]
    pub(crate) fn record_flow_consumed(bytes: u64) {
        FLOW_BYTES_CONSUMED.fetch_add(bytes, Ordering::Relaxed);
    }

    #[inline]
    pub(crate) fn record_flow_connection_update() {
        FLOW_CONNECTION_UPDATES.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub(crate) fn record_flow_stream_update() {
        FLOW_STREAM_UPDATES.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub(crate) fn record_scheduler_enqueue(priority: SchedulerPriority) {
        match priority {
            SchedulerPriority::Control => {
                SCHEDULER_CONTROL_ENQUEUED.fetch_add(1, Ordering::Relaxed);
            }
            SchedulerPriority::Interactive => {
                SCHEDULER_INTERACTIVE_ENQUEUED.fetch_add(1, Ordering::Relaxed);
            }
            SchedulerPriority::Bulk => {
                SCHEDULER_BULK_ENQUEUED.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    #[inline]
    pub(crate) fn record_scheduler_dequeue(priority: SchedulerPriority) {
        match priority {
            SchedulerPriority::Control => {
                SCHEDULER_CONTROL_DEQUEUED.fetch_add(1, Ordering::Relaxed);
            }
            SchedulerPriority::Interactive => {
                SCHEDULER_INTERACTIVE_DEQUEUED.fetch_add(1, Ordering::Relaxed);
            }
            SchedulerPriority::Bulk => {
                SCHEDULER_BULK_DEQUEUED.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    #[inline]
    pub(crate) fn totals() -> MetricsSnapshot {
        MetricsSnapshot {
            total_messages: TOTAL_MESSAGES.load(Ordering::Relaxed),
            sent_messages: SENT_MESSAGES.load(Ordering::Relaxed),
            received_messages: RECEIVED_MESSAGES.load(Ordering::Relaxed),
            total_errors: ERROR_COUNT.load(Ordering::Relaxed),
            active_connections: ACTIVE_CONNECTIONS.load(Ordering::Relaxed),
            active_streams: ACTIVE_STREAMS.load(Ordering::Relaxed),
            send_latency_total_ns: SEND_LATENCY_TOTAL_NS.load(Ordering::Relaxed),
            send_latency_max_ns: SEND_LATENCY_MAX_NS.load(Ordering::Relaxed),
            recv_latency_total_ns: RECV_LATENCY_TOTAL_NS.load(Ordering::Relaxed),
            recv_latency_max_ns: RECV_LATENCY_MAX_NS.load(Ordering::Relaxed),
            datagram_enqueued: DATAGRAM_ENQUEUED.load(Ordering::Relaxed),
            datagram_enqueued_bytes: DATAGRAM_ENQUEUED_BYTES.load(Ordering::Relaxed),
            datagram_sent: DATAGRAM_SENT.load(Ordering::Relaxed),
            datagram_sent_bytes: DATAGRAM_SENT_BYTES.load(Ordering::Relaxed),
            scheduler_control_enqueued: SCHEDULER_CONTROL_ENQUEUED.load(Ordering::Relaxed),
            scheduler_control_dequeued: SCHEDULER_CONTROL_DEQUEUED.load(Ordering::Relaxed),
            scheduler_interactive_enqueued: SCHEDULER_INTERACTIVE_ENQUEUED
                .load(Ordering::Relaxed),
            scheduler_interactive_dequeued: SCHEDULER_INTERACTIVE_DEQUEUED
                .load(Ordering::Relaxed),
            scheduler_bulk_enqueued: SCHEDULER_BULK_ENQUEUED.load(Ordering::Relaxed),
            scheduler_bulk_dequeued: SCHEDULER_BULK_DEQUEUED.load(Ordering::Relaxed),
            flow_bytes_consumed: FLOW_BYTES_CONSUMED.load(Ordering::Relaxed),
            flow_connection_updates: FLOW_CONNECTION_UPDATES.load(Ordering::Relaxed),
            flow_stream_updates: FLOW_STREAM_UPDATES.load(Ordering::Relaxed),
        }
    }
}

fn update_max(target: &AtomicU64, candidate: u64) {
    let mut current = target.load(Ordering::Relaxed);
    while candidate > current {
        match target.compare_exchange_weak(current, candidate, Ordering::Relaxed, Ordering::Relaxed)
        {
            Ok(_) => return,
            Err(old) => current = old,
        }
    }
}

/// Lightweight snapshot of critical counters.
#[derive(Default, Debug, Clone, Copy)]
pub struct MetricsSnapshot {
    pub total_messages: u64,
    pub sent_messages: u64,
    pub received_messages: u64,
    pub total_errors: u64,
    pub active_connections: u64,
    pub active_streams: u64,
    pub send_latency_total_ns: u64,
    pub send_latency_max_ns: u64,
    pub recv_latency_total_ns: u64,
    pub recv_latency_max_ns: u64,
    pub datagram_enqueued: u64,
    pub datagram_enqueued_bytes: u64,
    pub datagram_sent: u64,
    pub datagram_sent_bytes: u64,
    pub scheduler_control_enqueued: u64,
    pub scheduler_control_dequeued: u64,
    pub scheduler_interactive_enqueued: u64,
    pub scheduler_interactive_dequeued: u64,
    pub scheduler_bulk_enqueued: u64,
    pub scheduler_bulk_dequeued: u64,
    pub flow_bytes_consumed: u64,
    pub flow_connection_updates: u64,
    pub flow_stream_updates: u64,
}

impl MetricsSnapshot {
    /// Average send latency in microseconds.
    #[must_use]
    pub fn avg_send_latency_us(&self) -> Option<u64> {
        average_microseconds(self.send_latency_total_ns, self.sent_messages)
    }

    /// Average receive latency in microseconds.
    #[must_use]
    pub fn avg_receive_latency_us(&self) -> Option<u64> {
        average_microseconds(self.recv_latency_total_ns, self.received_messages)
    }
}

fn average_microseconds(total_ns: u64, count: u64) -> Option<u64> {
    if count == 0 {
        return None;
    }

    let total_ns_u128 = u128::from(total_ns);
    Some((total_ns_u128 / (u128::from(count) * NANOSECONDS_PER_MICROSECOND)) as u64)
}
