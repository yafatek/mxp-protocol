//! Flow control tracking for MXP transport streams and connections.

use std::collections::HashMap;

use super::stream::StreamId;

/// Errors related to flow control bookkeeping.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FlowControlError {
    /// Attempted to send more data than allowed by the advertised window.
    #[error("flow control limit exceeded: attempted {attempted} bytes with {available} bytes available")]
    SendWindowExceeded {
        /// Number of bytes available in the window.
        available: u64,
        /// Number of bytes the caller attempted to consume.
        attempted: u64,
    },
}

/// Sliding window tracking consumed bytes against a maximum allowance.
#[derive(Debug, Clone)]
pub struct FlowWindow {
    max_data: u64,
    consumed: u64,
}

impl FlowWindow {
    /// Create a new window with the specified limit.
    #[must_use]
    pub const fn new(max_data: u64) -> Self {
        Self {
            max_data,
            consumed: 0,
        }
    }

    /// Increase the window, e.g. when the peer advertises a new MAX_DATA value.
    pub fn update_limit(&mut self, new_max: u64) {
        if new_max > self.max_data {
            self.max_data = new_max;
        }
    }

    /// Remaining bytes that may be sent before hitting the limit.
    #[must_use]
    pub fn available(&self) -> u64 {
        self.max_data.saturating_sub(self.consumed)
    }

    /// Record consumption of bytes from the window.
    pub fn consume(&mut self, amount: u64) -> Result<(), FlowControlError> {
        let available = self.available();
        if amount > available {
            return Err(FlowControlError::SendWindowExceeded {
                available,
                attempted: amount,
            });
        }
        self.consumed = self.consumed.saturating_add(amount);
        Ok(())
    }

    /// Returns total bytes consumed so far.
    #[must_use]
    pub const fn consumed(&self) -> u64 {
        self.consumed
    }

    /// Current limit advertised for this window.
    #[must_use]
    pub const fn limit(&self) -> u64 {
        self.max_data
    }
}

/// Flow control management for connection-level and per-stream accounting.
#[derive(Debug, Default)]
pub struct FlowController {
    connection: FlowWindow,
    streams: HashMap<StreamId, FlowWindow>,
}

impl FlowController {
    /// Create a new controller with a connection-wide limit.
    #[must_use]
    pub fn new(connection_limit: u64) -> Self {
        Self {
            connection: FlowWindow::new(connection_limit),
            streams: HashMap::new(),
        }
    }

    /// Update the connection-wide limit.
    pub fn update_connection_limit(&mut self, new_limit: u64) {
        self.connection.update_limit(new_limit);
    }

    /// Acquire mutable reference to a stream-specific window, creating if absent.
    fn stream_window_mut(&mut self, id: StreamId) -> &mut FlowWindow {
        self.streams
            .entry(id)
            .or_insert_with(|| FlowWindow::new(self.connection.limit()))
    }

    /// Update the limit for a specific stream.
    pub fn update_stream_limit(&mut self, id: StreamId, new_limit: u64) {
        self.stream_window_mut(id).update_limit(new_limit);
    }

    /// Consume bytes from both connection-wide and stream-specific windows.
    pub fn consume(&mut self, id: StreamId, amount: u64) -> Result<(), FlowControlError> {
        let conn_available = self.connection.available();
        if amount > conn_available {
            return Err(FlowControlError::SendWindowExceeded {
                available: conn_available,
                attempted: amount,
            });
        }

        let stream_available = self.stream_window_mut(id).available();
        if amount > stream_available {
            return Err(FlowControlError::SendWindowExceeded {
                available: stream_available,
                attempted: amount,
            });
        }

        self.connection.consume(amount).expect("bounds checked");
        self.stream_window_mut(id)
            .consume(amount)
            .expect("bounds checked");
        Ok(())
    }

    /// Determine connection-level send availability.
    #[must_use]
    pub fn connection_available(&self) -> u64 {
        self.connection.available()
    }

    /// Determine per-stream send availability.
    #[must_use]
    pub fn stream_available(&self, id: StreamId) -> u64 {
        self.streams
            .get(&id)
            .map_or(self.connection.available(), FlowWindow::available)
    }

    /// Access the current connection limit.
    #[must_use]
    pub const fn connection_limit(&self) -> u64 {
        self.connection.limit()
    }

    /// Reset consumption counters (e.g., after receiving MAX_DATA that surpasses current total consumption).
    pub fn retire_connection_consumed(&mut self, amount: u64) {
        self.connection.consumed = self.connection.consumed.saturating_sub(amount);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flow_window_enforces_limits() {
        let mut window = FlowWindow::new(100);
        assert_eq!(window.available(), 100);
        window.consume(60).unwrap();
        assert_eq!(window.available(), 40);
        assert!(matches!(
            window.consume(50),
            Err(FlowControlError::SendWindowExceeded { available: 40, .. })
        ));
        window.update_limit(150);
        assert_eq!(window.available(), 90);
    }

    #[test]
    fn controller_tracks_connection_and_stream() {
        let mut controller = FlowController::new(200);
        let stream = StreamId::from_raw(0);
        controller.update_stream_limit(stream, 120);
        assert_eq!(controller.connection_available(), 200);
        assert_eq!(controller.stream_available(stream), 120);
        controller.consume(stream, 100).unwrap();
        assert_eq!(controller.connection_available(), 100);
        assert_eq!(controller.stream_available(stream), 20);
    }
}

