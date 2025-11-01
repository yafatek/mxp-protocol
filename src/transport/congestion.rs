//! Congestion control primitives for MXP transport (BBR-inspired).

use crate::transport::loss::{AckOutcome, SentPacketInfo};
use core::fmt;
use std::time::{Duration, SystemTime};
/// Gain cycle used by the pacing model (similar to BBR's 8-phase cycle).
const PACING_GAINS: [f64; 8] = [1.25, 1.0, 1.0, 1.0, 1.0, 1.0, 0.75, 1.0];

/// Configurable parameters for congestion control.
#[derive(Debug, Clone)]
pub struct CongestionConfig {
    /// Initial congestion window in bytes.
    pub initial_window: usize,
    /// Minimum congestion window in bytes.
    pub min_window: usize,
    /// Maximum allowed congestion window.
    pub max_window: usize,
    /// Minimum pacing rate in bytes per second.
    pub min_pacing_rate: f64,
    /// Maximum pacing rate in bytes per second.
    pub max_pacing_rate: f64,
}

impl Default for CongestionConfig {
    fn default() -> Self {
        Self {
            initial_window: 32 * 1024,
            min_window: 4 * 1024,
            max_window: 4 * 1024 * 1024,
            min_pacing_rate: 1_000.0,
            max_pacing_rate: 400_000_000.0,
        }
    }
}

/// Congestion control state machine.
#[derive(Debug)]
pub struct CongestionController {
    config: CongestionConfig,
    inflight_bytes: usize,
    congestion_window: usize,
    pacing_rate: f64,
    bandwidth_estimate: f64,
    cycle_index: usize,
    last_cycle_start: Option<SystemTime>,
    max_inflight: usize,
}

impl CongestionController {
    /// Create a new controller with default pacing rate.
    #[must_use]
    pub fn new(config: CongestionConfig) -> Self {
        let mut controller = Self {
            congestion_window: config.initial_window,
            pacing_rate: config.min_pacing_rate,
            inflight_bytes: 0,
            bandwidth_estimate: config.min_pacing_rate,
            cycle_index: 0,
            last_cycle_start: None,
            max_inflight: config.initial_window,
            config,
        };
        controller.recompute_pacing();
        controller
    }

    /// Called when a packet is sent.
    pub fn on_packet_sent(&mut self, size: usize) {
        self.inflight_bytes = self.inflight_bytes.saturating_add(size);
        self.max_inflight = self.max_inflight.max(self.inflight_bytes);
    }

    /// Called when ACK/loss info is available.
    pub fn on_ack_outcome(&mut self, outcome: &AckOutcome, now: SystemTime) {
        for pkt in &outcome.acknowledged {
            self.inflight_bytes = self.inflight_bytes.saturating_sub(pkt.size());
        }

        if !outcome.acknowledged.is_empty() {
            if let Some(rtt) = outcome.rtt_sample {
                if rtt > Duration::from_micros(0) {
                    let delivered: usize =
                        outcome.acknowledged.iter().map(SentPacketInfo::size).sum();
                    let seconds = duration_to_secs(rtt);
                    let bw = delivered as f64 / seconds.max(1e-9);
                    self.bandwidth_estimate = self.bandwidth_estimate.max(bw);
                }
            }
            self.increase_window();
        }

        if !outcome.lost.is_empty() {
            self.reduce_window();
        }

        self.advance_pacing_cycle(now);
        self.recompute_pacing();
    }

    /// Bytes currently permitted in flight.
    #[must_use]
    pub fn window(&self) -> usize {
        self.congestion_window
    }

    /// Suggested pacing rate in bytes per second.
    #[must_use]
    pub fn pacing_rate(&self) -> f64 {
        self.pacing_rate
    }

    /// Maximum number of bytes considered safely in-flight.
    #[must_use]
    pub fn max_inflight(&self) -> usize {
        self.max_inflight
    }

    fn increase_window(&mut self) {
        self.congestion_window = (self.congestion_window + 1500).min(self.config.max_window);
    }

    fn reduce_window(&mut self) {
        self.congestion_window = (self.congestion_window / 2).max(self.config.min_window);
        self.inflight_bytes = self.inflight_bytes.min(self.congestion_window);
    }

    fn advance_pacing_cycle(&mut self, now: SystemTime) {
        let cycle_duration = Duration::from_millis(55);
        match self.last_cycle_start {
            None => {
                self.last_cycle_start = Some(now);
                self.cycle_index = 0;
            }
            Some(start) if now.duration_since(start).unwrap_or_default() >= cycle_duration => {
                self.cycle_index = (self.cycle_index + 1) % PACING_GAINS.len();
                self.last_cycle_start = Some(now);
            }
            _ => {}
        }
    }

    fn recompute_pacing(&mut self) {
        let base_rate = self.bandwidth_estimate.max(self.config.min_pacing_rate);
        let gain = PACING_GAINS[self.cycle_index];
        let mut rate = base_rate * gain;
        rate = rate
            .min(self.config.max_pacing_rate)
            .max(self.config.min_pacing_rate);
        self.pacing_rate = rate;
    }
}

fn duration_to_secs(d: Duration) -> f64 {
    d.as_secs() as f64 + f64::from(d.subsec_nanos()) / 1_000_000_000.0
}

impl fmt::Display for CongestionController {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "cwnd={} inflight={} pacing={:.0}bps bw_est={:.0}bps",
            self.congestion_window, self.inflight_bytes, self.pacing_rate, self.bandwidth_estimate
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ack_pkt(number: u64, size: usize, sent: SystemTime) -> SentPacketInfo {
        SentPacketInfo::new(number, sent, size, true)
    }

    #[test]
    fn controller_increases_window_on_ack() {
        let config = CongestionConfig::default();
        let mut cc = CongestionController::new(config.clone());
        cc.on_packet_sent(1200);
        let now = SystemTime::now();
        let ack = AckOutcome {
            acknowledged: vec![ack_pkt(1, 1200, now - Duration::from_millis(10))],
            lost: Vec::new(),
            rtt_sample: Some(Duration::from_millis(10)),
        };
        cc.on_ack_outcome(&ack, now);
        assert!(cc.window() > config.initial_window);
        assert!(cc.pacing_rate() >= config.min_pacing_rate);
    }

    #[test]
    fn controller_reduces_window_on_loss() {
        let config = CongestionConfig::default();
        let mut cc = CongestionController::new(config.clone());
        for _ in 0..4 {
            cc.on_packet_sent(1200);
        }
        let now = SystemTime::now();
        let loss = AckOutcome {
            acknowledged: Vec::new(),
            lost: vec![ack_pkt(1, 1200, now - Duration::from_millis(5))],
            rtt_sample: None,
        };
        let prev_window = cc.window();
        cc.on_ack_outcome(&loss, now);
        assert!(cc.window() < prev_window);
        assert!(cc.window() >= config.min_window);
    }

    #[test]
    fn pacing_cycle_advances_over_time() {
        let config = CongestionConfig::default();
        let mut cc = CongestionController::new(config);
        let base = SystemTime::now();
        let ack = AckOutcome {
            acknowledged: vec![ack_pkt(1, 1200, base - Duration::from_millis(10))],
            lost: Vec::new(),
            rtt_sample: Some(Duration::from_millis(10)),
        };
        cc.on_ack_outcome(&ack, base);
        let first_rate = cc.pacing_rate();
        cc.on_ack_outcome(&ack, base + Duration::from_millis(60));
        let second_rate = cc.pacing_rate();
        assert_ne!(first_rate, second_rate);
    }
}
