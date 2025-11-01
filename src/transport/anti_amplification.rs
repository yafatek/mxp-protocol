//! Anti-amplification budget tracking for MXP transport handshakes.

/// Default amplification limit multiplier (3x per QUIC guidance).
pub const DEFAULT_AMPLIFICATION_FACTOR: usize = 3;

/// Configuration for the amplification guard.
#[derive(Debug, Clone)]
pub struct AmplificationConfig {
    /// Multiplier applied to received bytes to determine budget.
    pub factor: usize,
    /// Initial allowance (e.g. for stateless retry or version negotiation).
    pub initial_allowance: usize,
}

impl Default for AmplificationConfig {
    fn default() -> Self {
        Self {
            factor: DEFAULT_AMPLIFICATION_FACTOR,
            initial_allowance: 1200 * DEFAULT_AMPLIFICATION_FACTOR,
        }
    }
}

/// Tracks bytes observed and sent prior to handshake confirmation.
#[derive(Debug, Clone)]
pub struct AntiAmplificationGuard {
    config: AmplificationConfig,
    received: usize,
    sent: usize,
    verified: bool,
}

impl AntiAmplificationGuard {
    /// Construct a new guard with the provided configuration.
    #[must_use]
    pub fn new(config: AmplificationConfig) -> Self {
        Self {
            received: 0,
            sent: 0,
            verified: false,
            config,
        }
    }

    /// Record bytes received from the peer.
    pub fn on_receive(&mut self, bytes: usize) {
        self.received = self.received.saturating_add(bytes);
    }

    /// Attempt to reserve capacity for sending `bytes`. Returns `true` if permitted.
    pub fn try_consume(&mut self, bytes: usize) -> bool {
        if self.verified {
            self.sent = self.sent.saturating_add(bytes);
            return true;
        }

        let budget = self.available_budget();
        if bytes <= budget {
            self.sent = self.sent.saturating_add(bytes);
            true
        } else {
            false
        }
    }

    /// Mark the peer as verified (e.g. handshake complete), lifting restrictions.
    pub fn mark_verified(&mut self) {
        self.verified = true;
    }

    /// Determine how many additional bytes may be sent under current budget.
    #[must_use]
    pub fn available_budget(&self) -> usize {
        if self.verified {
            usize::MAX
        } else {
            let allowance = self
                .received
                .saturating_mul(self.config.factor)
                .saturating_add(self.config.initial_allowance);
            allowance.saturating_sub(self.sent)
        }
    }

    /// Check whether the amplification guard is still active.
    #[must_use]
    pub fn is_restricted(&self) -> bool {
        !self.verified
    }

    /// Bytes received so far.
    #[must_use]
    pub const fn received(&self) -> usize {
        self.received
    }

    /// Bytes sent so far.
    #[must_use]
    pub const fn sent(&self) -> usize {
        self.sent
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guard_blocks_over_budget_sends() {
        let mut guard = AntiAmplificationGuard::new(AmplificationConfig::default());
        assert!(guard.try_consume(1200));
        assert!(!guard.try_consume(4000));
        guard.on_receive(2000);
        assert!(guard.try_consume(4000));
    }

    #[test]
    fn guard_lifts_after_verification() {
        let mut guard = AntiAmplificationGuard::new(AmplificationConfig::default());
        assert!(guard.is_restricted());
        assert!(guard.try_consume(600));
        guard.mark_verified();
        assert!(!guard.is_restricted());
        assert!(guard.try_consume(1_000_000));
    }

    #[test]
    fn budget_accounts_for_initial_allowance() {
        let config = AmplificationConfig {
            initial_allowance: 0,
            ..Default::default()
        };
        let mut guard = AntiAmplificationGuard::new(config);
        assert!(!guard.try_consume(1));
        guard.on_receive(1000);
        assert!(guard.try_consume(2999));
        assert!(!guard.try_consume(2));
    }
}
