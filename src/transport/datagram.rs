//! Unreliable datagram queue with amplification guard integration.

use std::collections::VecDeque;

use super::anti_amplification::AntiAmplificationGuard;

#[cfg(test)]
use super::anti_amplification::AmplificationConfig;

/// Default maximum datagram payload size (bytes).
pub const DEFAULT_DATAGRAM_MAX_PAYLOAD: usize = 1200;
/// Default maximum number of queued datagrams.
pub const DEFAULT_DATAGRAM_QUEUE: usize = 256;

/// Configuration for datagram transmission.
#[derive(Debug, Clone)]
pub struct DatagramConfig {
    /// Maximum payload length permitted for a single datagram.
    pub max_payload: usize,
    /// Maximum number of datagrams held in the queue.
    pub max_queue: usize,
}

impl Default for DatagramConfig {
    fn default() -> Self {
        Self {
            max_payload: DEFAULT_DATAGRAM_MAX_PAYLOAD,
            max_queue: DEFAULT_DATAGRAM_QUEUE,
        }
    }
}

/// Errors produced by the datagram queue.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum DatagramError {
    /// Payload exceeds the configured maximum length.
    #[error("datagram payload too large: {len} bytes (max {max})")]
    PayloadTooLarge {
        /// Size of the attempted datagram payload.
        len: usize,
        /// Maximum size permitted by configuration.
        max: usize,
    },
    /// Queue is at capacity.
    #[error("datagram queue full (capacity {capacity})")]
    QueueFull {
        /// Configured maximum number of queued datagrams.
        capacity: usize,
    },
}

/// Manage outbound datagram payloads with amplification awareness.
#[derive(Debug)]
pub struct DatagramQueue {
    config: DatagramConfig,
    queue: VecDeque<Vec<u8>>,
}

impl DatagramQueue {
    /// Construct a queue using the provided configuration.
    #[must_use]
    pub fn new(config: DatagramConfig) -> Self {
        Self {
            queue: VecDeque::with_capacity(config.max_queue.min(64)),
            config,
        }
    }

    /// Enqueue a datagram payload.
    pub fn enqueue(&mut self, payload: Vec<u8>) -> Result<(), DatagramError> {
        if payload.len() > self.config.max_payload {
            return Err(DatagramError::PayloadTooLarge {
                len: payload.len(),
                max: self.config.max_payload,
            });
        }
        if self.queue.len() >= self.config.max_queue {
            return Err(DatagramError::QueueFull {
                capacity: self.config.max_queue,
            });
        }
        self.queue.push_back(payload);
        Ok(())
    }

    /// Returns number of queued datagrams awaiting transmission.
    #[must_use]
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Determine whether the queue holds no datagrams.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Attempt to dequeue a datagram when amplification budget permits.
    pub fn dequeue_with_guard(&mut self, guard: &mut AntiAmplificationGuard) -> Option<Vec<u8>> {
        let payload = self.queue.front()?;
        if guard.try_consume(payload.len()) {
            self.queue.pop_front()
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enqueue_respects_limits() {
        let mut queue = DatagramQueue::new(DatagramConfig {
            max_payload: 10,
            max_queue: 2,
        });
        assert!(queue.enqueue(vec![0; 5]).is_ok());
        assert!(matches!(
            queue.enqueue(vec![0; 11]),
            Err(DatagramError::PayloadTooLarge { .. })
        ));
        assert!(queue.enqueue(vec![1; 5]).is_ok());
        assert!(matches!(
            queue.enqueue(vec![2; 5]),
            Err(DatagramError::QueueFull { .. })
        ));
    }

    #[test]
    fn guard_allows_budgeted_send() {
        let mut queue = DatagramQueue::new(DatagramConfig::default());
        queue.enqueue(vec![1; 100]).unwrap();
        let mut guard = AntiAmplificationGuard::new(AmplificationConfig::default());
        guard.on_receive(1000);
        assert!(queue.dequeue_with_guard(&mut guard).is_some());
    }

    #[test]
    fn guard_blocks_when_budget_exhausted() {
        let mut queue = DatagramQueue::new(DatagramConfig::default());
        queue.enqueue(vec![1; 100]).unwrap();
        let mut guard = AntiAmplificationGuard::new(AmplificationConfig {
            initial_allowance: 0,
            ..AmplificationConfig::default()
        });
        assert!(queue.dequeue_with_guard(&mut guard).is_none());
    }
}
