//! ACK frame encoding, decoding, and receive history tracking for MXP transport.

use core::fmt;
use std::cmp::{max, min};
use std::time::{Duration, SystemTime};

/// Maximum number of ACK ranges tracked by default.
pub const DEFAULT_MAX_ACK_RANGES: usize = 32;

/// Error type for ACK frame processing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AckError {
    /// Input buffer too small for the expected structure.
    BufferTooSmall {
        /// Number of bytes required for decoding.
        expected: usize,
        /// Number of bytes actually provided by the caller.
        actual: usize,
    },
    /// ACK range has invalid ordering.
    InvalidRange {
        /// Lower bound of the range.
        start: u64,
        /// Upper bound of the range.
        end: u64,
    },
    /// Declared range count exceeds encoded payload.
    RangeCountMismatch,
    /// ACK history is empty when attempting to encode.
    EmptyHistory,
    /// Attempted to decode an ACK from a non-ACK frame.
    UnexpectedFrameType,
}

impl fmt::Display for AckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BufferTooSmall { expected, actual } => write!(
                f,
                "buffer too small for ACK frame: need {expected} bytes, have {actual}"
            ),
            Self::InvalidRange { start, end } => {
                write!(f, "invalid ACK range: start {start} > end {end}")
            }
            Self::RangeCountMismatch => write!(f, "encoded ACK ranges do not match declared count"),
            Self::EmptyHistory => write!(f, "no ACK ranges available to encode"),
            Self::UnexpectedFrameType => write!(f, "frame type is not ACK"),
        }
    }
}

impl std::error::Error for AckError {}

/// Inclusive ACK range (start <= end).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AckRange {
    start: u64,
    end: u64,
}

impl AckRange {
    /// Create a range ensuring start <= end.
    pub fn new(start: u64, end: u64) -> Result<Self, AckError> {
        if start > end {
            return Err(AckError::InvalidRange { start, end });
        }
        Ok(Self { start, end })
    }

    /// Range start accessor.
    #[must_use]
    pub const fn start(&self) -> u64 {
        self.start
    }

    /// Range end accessor.
    #[must_use]
    pub const fn end(&self) -> u64 {
        self.end
    }

    fn overlaps_or_adjacent(&self, other: &Self) -> bool {
        !(self.end.saturating_add(1) < other.start || other.end.saturating_add(1) < self.start)
    }

    fn merge(&self, other: &Self) -> Self {
        Self {
            start: min(self.start, other.start),
            end: max(self.end, other.end),
        }
    }
}

/// Encoded ACK frame containing the largest acknowledged packet and ranges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AckFrame {
    largest: u64,
    ack_delay_micros: u64,
    ranges: Vec<AckRange>,
}

impl AckFrame {
    /// Construct from components.
    pub fn new(
        largest: u64,
        ack_delay: Duration,
        mut ranges: Vec<AckRange>,
    ) -> Result<Self, AckError> {
        if ranges.is_empty() {
            return Err(AckError::EmptyHistory);
        }
        ranges.sort_by(|a, b| b.end.cmp(&a.end));
        if ranges[0].end != largest {
            return Err(AckError::InvalidRange {
                start: ranges[0].start(),
                end: ranges[0].end(),
            });
        }
        let ack_delay_micros = u64::try_from(ack_delay.as_micros().min(u128::from(u64::MAX)))
            .unwrap_or(u64::MAX);
        Ok(Self {
            largest,
            ack_delay_micros,
            ranges,
        })
    }

    /// Largest acknowledged packet number.
    #[must_use]
    pub const fn largest(&self) -> u64 {
        self.largest
    }

    /// ACK delay in microseconds.
    #[must_use]
    pub const fn ack_delay_micros(&self) -> u64 {
        self.ack_delay_micros
    }

    /// Borrow the range list (sorted descending by end).
    #[must_use]
    pub fn ranges(&self) -> &[AckRange] {
        &self.ranges
    }

    /// Encode into the provided buffer, appending bytes.
    pub fn encode(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.largest.to_le_bytes());
        out.extend_from_slice(&self.ack_delay_micros.to_le_bytes());
        let range_count = u16::try_from(self.ranges.len()).unwrap_or(u16::MAX);
        out.extend_from_slice(&range_count.to_le_bytes());
        for range in &self.ranges {
            out.extend_from_slice(&range.start.to_le_bytes());
            out.extend_from_slice(&range.end.to_le_bytes());
        }
    }

    /// Decode an ACK frame from bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self, AckError> {
        const HEADER_LEN: usize = 8 + 8 + 2;
        if bytes.len() < HEADER_LEN {
            return Err(AckError::BufferTooSmall {
                expected: HEADER_LEN,
                actual: bytes.len(),
            });
        }
        let largest = u64::from_le_bytes(bytes[0..8].try_into().unwrap());
        let ack_delay_micros = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
        let range_count = u16::from_le_bytes(bytes[16..18].try_into().unwrap()) as usize;

        let mut offset = HEADER_LEN;
        let mut ranges = Vec::with_capacity(range_count);
        for _ in 0..range_count {
            if bytes.len() < offset + 16 {
                return Err(AckError::RangeCountMismatch);
            }
            let start = u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
            offset += 8;
            let end = u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
            offset += 8;
            ranges.push(AckRange::new(start, end)?);
        }

        if ranges.is_empty() {
            return Err(AckError::EmptyHistory);
        }
        ranges.sort_by(|a, b| b.end.cmp(&a.end));
        if ranges[0].end != largest {
            return Err(AckError::InvalidRange {
                start: ranges[0].start(),
                end: ranges[0].end(),
            });
        }

        Ok(Self {
            largest,
            ack_delay_micros,
            ranges,
        })
    }
}

/// Receive history used to build ACK frames for packets observed from the peer.
#[derive(Debug)]
pub struct ReceiveHistory {
    ranges: Vec<AckRange>,
    max_ranges: usize,
    ack_delay: Duration,
    last_ack_time: Option<SystemTime>,
    ack_request_time: Option<SystemTime>,
}

impl ReceiveHistory {
    /// Create a new history with configurable capacity and ACK delay target.
    #[must_use]
    pub fn new(max_ranges: usize, ack_delay: Duration) -> Self {
        Self {
            ranges: Vec::with_capacity(max_ranges),
            max_ranges: max_ranges.max(1),
            ack_delay,
            last_ack_time: None,
            ack_request_time: None,
        }
    }

    /// Observation of a packet number; returns true when an immediate ACK is suggested.
    pub fn record(&mut self, packet_number: u64, ack_eliciting: bool, now: SystemTime) -> bool {
        self.insert_packet(packet_number);
        if ack_eliciting && self.ack_request_time.is_none() {
            self.ack_request_time = Some(now);
        }

        self.should_ack_immediately(now)
    }

    /// Build an ACK frame if data is available.
    pub fn build_frame(&mut self, now: SystemTime) -> Result<Option<AckFrame>, AckError> {
        if self.ranges.is_empty() {
            return Ok(None);
        }

        let largest = self.ranges[0].end();
        let ack_delay = self
            .last_ack_time
            .map(|sent| {
                now.duration_since(sent)
                    .unwrap_or_else(|_| Duration::default())
            })
            .unwrap_or_default();
        let ranges = self.ranges.clone();
        let frame = AckFrame::new(largest, ack_delay, ranges)?;
        self.last_ack_time = Some(now);
        self.ack_request_time = None;
        Ok(Some(frame))
    }

    /// Expose current ranges for inspection/testing.
    #[must_use]
    pub fn ranges(&self) -> &[AckRange] {
        &self.ranges
    }

    fn should_ack_immediately(&self, now: SystemTime) -> bool {
        if let Some(requested) = self.ack_request_time {
            if let Ok(elapsed) = now.duration_since(requested) {
                return elapsed >= self.ack_delay;
            }
        }
        false
    }

    fn insert_packet(&mut self, packet_number: u64) {
        let mut inserted = false;
        for idx in 0..self.ranges.len() {
            let range = self.ranges[idx];
            if packet_number >= range.start && packet_number <= range.end {
                return; // already present
            }

            if packet_number.checked_add(1) == Some(range.start) {
                self.ranges[idx] = AckRange::new(packet_number, range.end).unwrap();
                self.compress_around(idx);
                inserted = true;
                break;
            }

            if range.end.checked_add(1) == Some(packet_number) {
                self.ranges[idx] = AckRange::new(range.start, packet_number).unwrap();
                self.compress_around(idx);
                inserted = true;
                break;
            }

            if packet_number > range.end {
                self.ranges
                    .insert(idx, AckRange::new(packet_number, packet_number).unwrap());
                inserted = true;
                break;
            }
        }

        if !inserted {
            self.ranges
                .push(AckRange::new(packet_number, packet_number).unwrap());
        }

        self.truncate_to_capacity();
    }

    fn compress_around(&mut self, idx: usize) {
        if idx > 0 {
            let current = self.ranges[idx];
            let prev = self.ranges[idx - 1];
            if current.overlaps_or_adjacent(&prev) {
                let merged = current.merge(&prev);
                self.ranges[idx - 1] = merged;
                self.ranges.remove(idx);
                self.compress_around(idx - 1);
                return;
            }
        }

        if idx + 1 < self.ranges.len() {
            let current = self.ranges[idx];
            let next = self.ranges[idx + 1];
            if current.overlaps_or_adjacent(&next) {
                let merged = current.merge(&next);
                self.ranges[idx] = merged;
                self.ranges.remove(idx + 1);
                self.compress_around(idx);
            }
        }
    }

    fn truncate_to_capacity(&mut self) {
        if self.ranges.len() <= self.max_ranges {
            return;
        }
        self.ranges.truncate(self.max_ranges);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ack_range_new_validates_ordering() {
        assert!(AckRange::new(1, 3).is_ok());
        assert!(matches!(
            AckRange::new(3, 1),
            Err(AckError::InvalidRange { .. })
        ));
    }

    #[test]
    fn ack_frame_encode_decode_roundtrip() {
        let ranges = vec![AckRange::new(10, 15).unwrap(), AckRange::new(3, 5).unwrap()];
        let frame = AckFrame::new(15, Duration::from_micros(250), ranges).unwrap();
        let mut buf = Vec::new();
        frame.encode(&mut buf);
        let decoded = AckFrame::decode(&buf).unwrap();
        assert_eq!(decoded.largest(), 15);
        assert_eq!(decoded.ack_delay_micros(), 250);
        assert_eq!(decoded.ranges().len(), 2);
        assert_eq!(decoded.ranges()[0], AckRange::new(10, 15).unwrap());
    }

    #[test]
    fn receive_history_merges_adjacent_packets() {
        let mut history = ReceiveHistory::new(8, Duration::from_millis(1));
        let now = SystemTime::now();
        history.record(5, true, now);
        history.record(4, true, now);
        history.record(7, true, now);
        history.record(6, true, now);
        assert_eq!(history.ranges().len(), 1);
        assert_eq!(history.ranges()[0], AckRange::new(4, 7).unwrap());
    }

    #[test]
    fn receive_history_limits_range_count() {
        let mut history = ReceiveHistory::new(2, Duration::from_millis(1));
        let now = SystemTime::now();
        history.record(10, true, now);
        history.record(8, true, now);
        history.record(6, true, now);
        assert!(history.ranges().len() <= 2);
    }

    #[test]
    fn receive_history_builds_ack_frame() {
        let mut history = ReceiveHistory::new(8, Duration::from_millis(0));
        let now = SystemTime::now();
        history.record(10, true, now);
        history.record(9, true, now);
        history.record(7, false, now);
        let frame = history.build_frame(now).unwrap().unwrap();
        assert_eq!(frame.largest(), 10);
        assert_eq!(frame.ranges().len(), 2);
        assert_eq!(frame.ranges()[0], AckRange::new(9, 10).unwrap());
    }
}
