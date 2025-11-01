//! Priority-aware scheduling for streams and datagrams.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, VecDeque};

use super::stream::StreamId;

#[cfg(test)]
use super::stream::{EndpointRole, StreamKind};
use crate::protocol::metrics::{self, SchedulerPriority};
use tracing::trace;

/// Priority class for outbound transmissions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PriorityClass {
    /// High-priority control/handshake traffic.
    Control,
    /// Latency-sensitive agent RPC.
    Interactive,
    /// Background or bulk transfers.
    Bulk,
}

impl PriorityClass {
    const fn weight(self) -> u32 {
        match self {
            Self::Control => 100,
            Self::Interactive => 50,
            Self::Bulk => 10,
        }
    }
}

/// Queue entry representing a stream ready to transmit.
#[derive(Debug)]
struct StreamEntry {
    weight: u32,
    sequence: u64,
    id: StreamId,
    priority: PriorityClass,
}

impl PartialEq for StreamEntry {
    fn eq(&self, other: &Self) -> bool {
        self.weight == other.weight && self.sequence == other.sequence && self.id == other.id
    }
}

impl Eq for StreamEntry {}

impl PartialOrd for StreamEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for StreamEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.weight.cmp(&other.weight) {
            Ordering::Equal => self.sequence.cmp(&other.sequence).reverse(),
            ordering => ordering,
        }
    }
}

/// Scheduler tracking active streams and datagram queue.
#[derive(Debug)]
pub struct Scheduler {
    streams: BinaryHeap<StreamEntry>,
    datagrams: VecDeque<Vec<u8>>,
    sequence: u64,
}

impl Scheduler {
    /// Construct an empty scheduler.
    #[must_use]
    pub fn new() -> Self {
        Self {
            streams: BinaryHeap::new(),
            datagrams: VecDeque::new(),
            sequence: 0,
        }
    }

    /// Register a stream ready to send.
    pub fn push_stream(&mut self, id: StreamId, priority: PriorityClass) {
        self.sequence = self.sequence.wrapping_add(1);
        trace!(
            stream = id.as_u64(),
            ?priority,
            "enqueue stream for scheduling"
        );
        metrics::Metrics::record_scheduler_enqueue(priority.into());
        self.streams.push(StreamEntry {
            priority,
            weight: priority.weight(),
            sequence: self.sequence,
            id,
        });
    }

    /// Register an outbound datagram payload.
    pub fn push_datagram(&mut self, payload: Vec<u8>) {
        trace!(len = payload.len(), "enqueue datagram");
        self.datagrams.push_back(payload);
    }

    /// Pop the highest priority stream, if any.
    pub fn pop_stream(&mut self) -> Option<(StreamId, PriorityClass)> {
        self.streams.pop().map(|entry| {
            trace!(stream = entry.id.as_u64(), ?entry.priority, "dequeue stream for transmit");
            metrics::Metrics::record_scheduler_dequeue(entry.priority.into());
            (entry.id, entry.priority)
        })
    }

    /// Pop the oldest datagram payload.
    pub fn pop_datagram(&mut self) -> Option<Vec<u8>> {
        let datagram = self.datagrams.pop_front();
        if let Some(ref payload) = datagram {
            trace!(len = payload.len(), "dequeue datagram");
        }
        datagram
    }

    /// Check whether any streams are queued.
    #[must_use]
    pub fn has_streams(&self) -> bool {
        !self.streams.is_empty()
    }

    /// Check whether datagrams are waiting to send.
    #[must_use]
    pub fn has_datagrams(&self) -> bool {
        !self.datagrams.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheduler_orders_by_priority() {
        let mut scheduler = Scheduler::new();
        let stream_a = StreamId::new(EndpointRole::Client, StreamKind::Bidirectional, 1);
        let stream_b = StreamId::new(EndpointRole::Client, StreamKind::Bidirectional, 2);
        scheduler.push_stream(stream_a, PriorityClass::Bulk);
        scheduler.push_stream(stream_b, PriorityClass::Control);
        let first = scheduler.pop_stream().expect("first");
        assert_eq!(first.0, stream_b);
        assert_eq!(first.1, PriorityClass::Control);
        let second = scheduler.pop_stream().expect("second");
        assert_eq!(second.0, stream_a);
    }

    #[test]
    fn datagram_queue_is_fifo() {
        let mut scheduler = Scheduler::new();
        scheduler.push_datagram(vec![1]);
        scheduler.push_datagram(vec![2]);
        assert_eq!(scheduler.pop_datagram().unwrap(), vec![1]);
        assert_eq!(scheduler.pop_datagram().unwrap(), vec![2]);
    }
}
