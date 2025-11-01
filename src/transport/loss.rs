//! Sent packet tracking, RTT estimation, and loss detection for MXP transport.

use crate::transport::ack::AckFrame;
use core::cmp::Ordering;
use std::collections::VecDeque;
use std::time::{Duration, SystemTime};
use tracing::{debug, trace};

/// Information about a sent packet retained for loss detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentPacketInfo {
    packet_number: u64,
    time_sent: SystemTime,
    size: usize,
    ack_eliciting: bool,
}

impl SentPacketInfo {
    /// Create a new sent packet record.
    #[must_use]
    pub fn new(
        packet_number: u64,
        time_sent: SystemTime,
        size: usize,
        ack_eliciting: bool,
    ) -> Self {
        Self {
            packet_number,
            time_sent,
            size,
            ack_eliciting,
        }
    }

    /// Packet number accessor.
    #[must_use]
    pub const fn packet_number(&self) -> u64 {
        self.packet_number
    }

    /// Time the packet left the socket.
    #[must_use]
    pub const fn time_sent(&self) -> SystemTime {
        self.time_sent
    }

    /// Payload size in bytes counted towards the congestion window.
    #[must_use]
    pub const fn size(&self) -> usize {
        self.size
    }

    /// Whether the packet was ack-eliciting.
    #[must_use]
    pub const fn ack_eliciting(&self) -> bool {
        self.ack_eliciting
    }
}

/// Summary of ACK processing.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct AckOutcome {
    /// Packets newly acknowledged by this ACK frame.
    pub acknowledged: Vec<SentPacketInfo>,
    /// Packets declared lost due to reordering threshold or time threshold.
    pub lost: Vec<SentPacketInfo>,
    /// Latest RTT sample derived from the ACK delay.
    pub rtt_sample: Option<Duration>,
}

/// Configurable parameters driving the loss detector.
#[derive(Debug, Clone)]
pub struct LossConfig {
    /// Packet threshold for declaring loss (QUIC default is 3).
    pub packet_threshold: u64,
    /// Factor applied to time threshold numerator (default 9).
    pub time_threshold_factor_numerator: u32,
    /// Factor applied to time threshold denominator (default 8).
    pub time_threshold_factor_denominator: u32,
    /// Initial RTT used before any samples are observed.
    pub initial_rtt: Duration,
    /// Maximum ACK delay we are willing to subtract from RTT samples.
    pub max_ack_delay: Duration,
}

impl Default for LossConfig {
    fn default() -> Self {
        Self {
            packet_threshold: 3,
            time_threshold_factor_numerator: 9,
            time_threshold_factor_denominator: 8,
            initial_rtt: Duration::from_millis(333),
            max_ack_delay: Duration::from_millis(25),
        }
    }
}

/// Tracks outstanding packets and estimates RTT/loss timers.
#[derive(Debug)]
pub struct LossManager {
    config: LossConfig,
    outstanding: VecDeque<SentPacketInternal>,
    largest_acked: Option<u64>,
    latest_rtt: Option<Duration>,
    smoothed_rtt: Option<Duration>,
    rtt_var: Option<Duration>,
    min_rtt: Option<Duration>,
    loss_time: Option<SystemTime>,
}

#[derive(Debug, Clone)]
struct SentPacketInternal {
    info: SentPacketInfo,
}

impl LossManager {
    /// Create a new manager with the provided configuration.
    #[must_use]
    pub fn new(config: LossConfig) -> Self {
        Self {
            config,
            outstanding: VecDeque::new(),
            largest_acked: None,
            latest_rtt: None,
            smoothed_rtt: None,
            rtt_var: None,
            min_rtt: None,
            loss_time: None,
        }
    }

    /// Record a packet that has just been sent.
    pub fn on_packet_sent(
        &mut self,
        packet_number: u64,
        time_sent: SystemTime,
        size: usize,
        ack_eliciting: bool,
    ) {
        trace!(
            packet_number,
            size, ack_eliciting, "loss tracker observe sent packet"
        );
        let info = SentPacketInfo::new(packet_number, time_sent, size, ack_eliciting);
        self.outstanding.push_back(SentPacketInternal { info });
        if ack_eliciting {
            self.update_loss_time(time_sent);
        }
    }

    /// Process an ACK frame received at `now`, returning ACK/loss outcomes.
    pub fn on_ack_frame(&mut self, frame: &AckFrame, now: SystemTime) -> AckOutcome {
        debug!(
            largest = frame.largest(),
            "loss tracker processing ACK frame"
        );
        let mut outcome = AckOutcome::default();

        let mut retained = VecDeque::with_capacity(self.outstanding.len());
        let mut acknowledged_largest: Option<SentPacketInfo> = None;

        for entry in self.outstanding.drain(..) {
            if ack_contains(frame, entry.info.packet_number) {
                if acknowledged_largest
                    .as_ref()
                    .is_none_or(|pkt| pkt.packet_number < entry.info.packet_number)
                {
                    acknowledged_largest = Some(entry.info.clone());
                }
                outcome.acknowledged.push(entry.info.clone());
            } else {
                retained.push_back(entry);
            }
        }

        self.outstanding = retained;

        if let Some(largest) = acknowledged_largest {
            self.largest_acked = Some(largest.packet_number);
            let ack_delay = Duration::from_micros(frame.ack_delay_micros());
            let ack_delay = ack_delay.min(self.config.max_ack_delay);
            if let Ok(mut latest) = now.duration_since(largest.time_sent) {
                // Subtract acknowledged ACK delay if it does not underflow.
                if latest > ack_delay {
                    latest -= ack_delay;
                }
                outcome.rtt_sample = Some(latest);
                self.update_rtt_estimates(latest);
            }
        }

        let lost = self.detect_losses(frame.largest(), now);
        outcome.lost.extend(lost);

        self.recalculate_loss_time(now);

        outcome
    }

    /// Query when the next loss timer should fire.
    #[must_use]
    pub const fn loss_time(&self) -> Option<SystemTime> {
        self.loss_time
    }

    /// Trigger time-based loss detection when the loss timer fires.
    pub fn on_loss_timeout(&mut self, now: SystemTime) -> Vec<SentPacketInfo> {
        match self.loss_time {
            Some(deadline) if deadline <= now => {}
            _ => return Vec::new(),
        }

        let Some(delay) = self.time_threshold() else {
            return Vec::new();
        };

        let mut lost = Vec::new();
        let mut retained = VecDeque::with_capacity(self.outstanding.len());

        for entry in self.outstanding.drain(..) {
            if !entry.info.ack_eliciting {
                retained.push_back(entry);
                continue;
            }

            let elapsed = now.duration_since(entry.info.time_sent).unwrap_or_default();
            if elapsed >= delay {
                debug!(
                    packet_number = entry.info.packet_number(),
                    "loss via explicit timeout"
                );
                lost.push(entry.info.clone());
            } else {
                retained.push_back(entry);
            }
        }

        self.outstanding = retained;
        self.recalculate_loss_time(now);
        lost
    }

    /// Latest RTT sample observed.
    #[must_use]
    pub const fn latest_rtt(&self) -> Option<Duration> {
        self.latest_rtt
    }

    /// Smoothed RTT estimate.
    #[must_use]
    pub const fn smoothed_rtt(&self) -> Option<Duration> {
        self.smoothed_rtt
    }

    /// RTT variation estimate.
    #[must_use]
    pub const fn rtt_variance(&self) -> Option<Duration> {
        self.rtt_var
    }

    /// Remaining outstanding packet references (for diagnostics).
    #[must_use]
    pub fn outstanding(&self) -> impl Iterator<Item = &SentPacketInfo> {
        self.outstanding.iter().map(|entry| &entry.info)
    }

    fn update_rtt_estimates(&mut self, latest: Duration) {
        self.latest_rtt = Some(latest);
        self.min_rtt = Some(self.min_rtt.map_or(latest, |min_rtt| min_rtt.min(latest)));

        match (self.smoothed_rtt, self.rtt_var) {
            (None, _) | (_, None) => {
                self.smoothed_rtt = Some(latest);
                self.rtt_var = Some(latest / 2);
            }
            (Some(srtt), Some(rttvar)) => {
                let abs_err = abs_duration_diff(srtt, latest);
                let new_var = ((3 * rttvar) + abs_err) / 4;
                let new_srtt = ((7 * srtt) + latest) / 8;
                self.rtt_var = Some(new_var.max(Duration::from_micros(1)));
                self.smoothed_rtt = Some(new_srtt.max(Duration::from_micros(1)));
            }
        }
    }

    fn detect_losses(&mut self, largest_acked: u64, now: SystemTime) -> Vec<SentPacketInfo> {
        let mut lost = Vec::new();
        let mut retained = VecDeque::with_capacity(self.outstanding.len());
        let threshold = self.config.packet_threshold;
        let loss_delay = self.time_threshold();

        for entry in self.outstanding.drain(..) {
            if largest_acked >= entry.info.packet_number
                && largest_acked - entry.info.packet_number >= threshold
            {
                debug!(
                    packet_number = entry.info.packet_number(),
                    "loss via packet threshold"
                );
                lost.push(entry.info.clone());
                continue;
            }

            if let Some(delay) = loss_delay {
                if now.duration_since(entry.info.time_sent).unwrap_or_default() >= delay {
                    debug!(
                        packet_number = entry.info.packet_number(),
                        "loss via time threshold"
                    );
                    lost.push(entry.info.clone());
                    continue;
                }
            }

            retained.push_back(entry);
        }

        self.outstanding = retained;
        lost
    }

    fn time_threshold(&self) -> Option<Duration> {
        let base = self
            .latest_rtt
            .or(self.smoothed_rtt)
            .unwrap_or(self.config.initial_rtt);
        Some(scale_duration(
            base,
            self.config.time_threshold_factor_numerator,
            self.config.time_threshold_factor_denominator,
        ))
    }

    fn update_loss_time(&mut self, now: SystemTime) {
        if self.loss_time.is_none() {
            if let Some(delay) = self.time_threshold() {
                self.loss_time = Some(now + delay);
            }
        }
    }

    fn recalculate_loss_time(&mut self, now: SystemTime) {
        self.loss_time = None;
        for entry in &self.outstanding {
            if !entry.info.ack_eliciting {
                continue;
            }
            if let Some(delay) = self.time_threshold() {
                let candidate = entry.info.time_sent + delay;
                self.loss_time = match self.loss_time {
                    Some(current) if current <= candidate => Some(current),
                    _ => Some(candidate),
                };
            }
        }

        if self.loss_time.is_some() {
            return;
        }

        if let Some(delay) = self.time_threshold() {
            self.loss_time = Some(now + delay);
        }
    }
}

fn ack_contains(frame: &AckFrame, packet_number: u64) -> bool {
    frame
        .ranges()
        .iter()
        .any(|range| packet_number >= range.start() && packet_number <= range.end())
}

fn abs_duration_diff(a: Duration, b: Duration) -> Duration {
    match a.cmp(&b) {
        Ordering::Less => b - a,
        Ordering::Greater => a - b,
        Ordering::Equal => Duration::from_secs(0),
    }
}

fn scale_duration(base: Duration, numerator: u32, denominator: u32) -> Duration {
    if denominator == 0 {
        return base;
    }
    let scaled = base.as_nanos() * u128::from(numerator) / u128::from(denominator);
    let capped = scaled.min(u128::from(u64::MAX));
    Duration::from_nanos(capped as u64).max(Duration::from_micros(1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::ack::AckRange;

    fn ack_frame_from_ranges(largest: u64, ack_delay: Duration, ranges: &[(u64, u64)]) -> AckFrame {
        let range_structs: Vec<AckRange> = ranges
            .iter()
            .map(|(start, end)| AckRange::new(*start, *end).unwrap())
            .collect();
        AckFrame::new(largest, ack_delay, range_structs).unwrap()
    }

    #[test]
    fn ack_marks_packets_acked_and_updates_rtt() {
        let mut mgr = LossManager::new(LossConfig::default());
        let send_time = SystemTime::now();
        mgr.on_packet_sent(10, send_time, 1200, true);
        let ack_time = send_time + Duration::from_millis(50);
        let frame = ack_frame_from_ranges(10, Duration::from_millis(10), &[(10, 10)]);
        let outcome = mgr.on_ack_frame(&frame, ack_time);
        assert_eq!(outcome.acknowledged.len(), 1);
        assert!(outcome.lost.is_empty());
        let sample = outcome.rtt_sample.expect("sample");
        assert!(sample >= Duration::from_millis(39));
        assert!(sample <= Duration::from_millis(40));
        assert!(mgr.latest_rtt().is_some());
    }

    #[test]
    fn packet_threshold_declares_loss() {
        let mut config = LossConfig {
            packet_threshold: 2,
            ..Default::default()
        };
        let mut mgr = LossManager::new(config);
        let base = SystemTime::now();
        mgr.on_packet_sent(1, base, 1000, true);
        mgr.on_packet_sent(2, base, 1000, true);
        mgr.on_packet_sent(3, base, 1000, true);
        mgr.on_packet_sent(4, base, 1000, true);

        let ack_time = base + Duration::from_millis(5);
        let frame = ack_frame_from_ranges(4, Duration::from_micros(0), &[(4, 4)]);
        let outcome = mgr.on_ack_frame(&frame, ack_time);

        assert_eq!(outcome.acknowledged.len(), 1);
        assert_eq!(outcome.lost.len(), 2);
        assert!(outcome.lost.iter().any(|pkt| pkt.packet_number() == 1));
        assert!(outcome.lost.iter().any(|pkt| pkt.packet_number() == 2));
    }

    #[test]
    fn time_threshold_declares_loss() {
        let mut config = LossConfig {
            initial_rtt: Duration::from_millis(5),
            ..Default::default()
        };
        let mut mgr = LossManager::new(config);
        let base = SystemTime::now();
        mgr.on_packet_sent(5, base, 900, true);

        let ack_time = base + Duration::from_millis(50);
        let frame = ack_frame_from_ranges(6, Duration::from_millis(0), &[(6, 6)]);
        let outcome = mgr.on_ack_frame(&frame, ack_time);
        assert!(!outcome.lost.is_empty());
    }

    #[test]
    fn loss_time_updates_on_send_and_ack() {
        let mut mgr = LossManager::new(LossConfig::default());
        let now = SystemTime::now();
        mgr.on_packet_sent(1, now, 1200, true);
        assert!(mgr.loss_time().is_some());
        let frame = ack_frame_from_ranges(1, Duration::from_millis(0), &[(1, 1)]);
        mgr.on_ack_frame(&frame, now + Duration::from_millis(30));
        assert!(mgr.loss_time().is_some());
    }
}
