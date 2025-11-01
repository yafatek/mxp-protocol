//! Reliable stream state machines and buffering for MXP transport.

use std::collections::{BTreeMap, HashMap, VecDeque};

use crate::protocol::metrics::Metrics;
use tracing::{debug, instrument, trace};

use super::flow::{FlowControlError, FlowController};

/// Direction of stream initiation relative to the local endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointRole {
    /// Local endpoint initiated the stream (client side by convention).
    Client,
    /// Remote endpoint initiated the stream (server side by convention).
    Server,
}

impl EndpointRole {
    const fn bit(self) -> u64 {
        match self {
            Self::Client => 0,
            Self::Server => 1,
        }
    }

    /// Determine role from encoded bit.
    const fn from_bit(bit: u64) -> Self {
        if bit == 0 { Self::Client } else { Self::Server }
    }
}

/// Stream directionality classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamKind {
    /// Bidirectional stream allowing simultaneous send/receive.
    Bidirectional,
    /// Unidirectional stream (send-only or receive-only depending on role).
    Unidirectional,
}

impl StreamKind {
    const fn bit(self) -> u64 {
        match self {
            Self::Bidirectional => 0,
            Self::Unidirectional => 1,
        }
    }

    const fn from_bit(bit: u64) -> Self {
        if bit == 0 {
            Self::Bidirectional
        } else {
            Self::Unidirectional
        }
    }
}

/// Monotonically increasing identifier for streams.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StreamId(u64);

impl StreamId {
    /// Compose a stream identifier from role, kind, and sequence number.
    #[must_use]
    pub const fn new(role: EndpointRole, kind: StreamKind, index: u64) -> Self {
        Self((index << 2) | (role.bit() << 1) | kind.bit())
    }

    /// Parse a raw identifier.
    #[must_use]
    pub const fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    /// Return the raw numeric value.
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    /// Sequence index of the stream.
    #[must_use]
    pub const fn index(self) -> u64 {
        self.0 >> 2
    }

    /// Role that initiated the stream.
    #[must_use]
    pub const fn role(self) -> EndpointRole {
        EndpointRole::from_bit((self.0 >> 1) & 1)
    }

    /// Stream kind (bidi / uni).
    #[must_use]
    pub const fn kind(self) -> StreamKind {
        StreamKind::from_bit(self.0 & 1)
    }

    /// Whether the local endpoint is the initiator given its role.
    #[must_use]
    pub fn is_local_initiated(self, local: EndpointRole) -> bool {
        self.role() == local
    }
}

/// Error conditions for stream operations.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum StreamError {
    /// Attempted to queue data after a local FIN has been sent.
    #[error("stream already finished locally")]
    AlreadyFinished,
    /// Received data beyond the declared final offset.
    #[error("data beyond final offset")]
    DataBeyondFinalOffset,
    /// Data overlaps an existing chunk with inconsistent payload.
    #[error("conflicting data for offset {offset}")]
    ConflictingData {
        /// Offset at which conflicting bytes were observed.
        offset: u64,
    },
    /// Stream doesn't present in the manager.
    #[error("unknown stream id")]
    UnknownStream,
}

/// Chunk of data ready for transmission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendChunk {
    /// Byte offset within the stream.
    pub offset: u64,
    /// Payload bytes to transmit (ownership transferred to caller).
    pub payload: Vec<u8>,
    /// Whether this chunk carries the final FIN flag.
    pub fin: bool,
}

#[derive(Debug, Default)]
struct SendBuffer {
    buffer: VecDeque<u8>,
    fin_queued: bool,
    fin_sent: bool,
    next_offset: u64,
}

impl SendBuffer {
    fn queue(&mut self, data: &[u8]) -> Result<(), StreamError> {
        if self.fin_queued {
            return Err(StreamError::AlreadyFinished);
        }
        self.buffer.extend(data);
        Ok(())
    }

    fn queue_fin(&mut self) -> Result<(), StreamError> {
        if self.fin_queued {
            Err(StreamError::AlreadyFinished)
        } else {
            self.fin_queued = true;
            Ok(())
        }
    }

    fn next_chunk(&mut self, max_len: usize) -> Option<SendChunk> {
        if (self.fin_sent || !self.fin_queued) && self.buffer.is_empty() {
            return None;
        }

        let take = self.buffer.len().min(max_len);
        let mut payload = Vec::with_capacity(take);
        for _ in 0..take {
            if let Some(byte) = self.buffer.pop_front() {
                payload.push(byte);
            }
        }

        let fin = self.buffer.is_empty() && self.fin_queued && !self.fin_sent;
        if fin {
            self.fin_sent = true;
        }
        let offset = self.next_offset;
        self.next_offset = self.next_offset.saturating_add(payload.len() as u64);

        Some(SendChunk {
            offset,
            payload,
            fin,
        })
    }

    fn is_drained(&self) -> bool {
        self.buffer.is_empty() && (!self.fin_queued || self.fin_sent)
    }
}

#[derive(Debug, Default)]
struct RecvBuffer {
    delivered_offset: u64,
    ready: VecDeque<u8>,
    pending: BTreeMap<u64, Vec<u8>>,
    final_offset: Option<u64>,
}

impl RecvBuffer {
    fn ingest(&mut self, offset: u64, data: &[u8], fin: bool) -> Result<(), StreamError> {
        if let Some(final_offset) = self.final_offset {
            let incoming_end = offset.saturating_add(data.len() as u64);
            if incoming_end > final_offset {
                return Err(StreamError::DataBeyondFinalOffset);
            }
        }

        if data.is_empty() && !fin {
            return Ok(());
        }

        let entry = self.pending.entry(offset).or_default();
        if entry.is_empty() {
            entry.extend_from_slice(data);
        } else if entry.as_slice() != data {
            return Err(StreamError::ConflictingData { offset });
        }

        if fin {
            let end = offset.saturating_add(data.len() as u64);
            self.final_offset = Some(self.final_offset.map_or(end, |prev| prev.max(end)));
        }

        self.promote_pending();
        Ok(())
    }

    fn promote_pending(&mut self) {
        loop {
            let next_offset = self.delivered_offset + self.ready.len() as u64;
            let Some((&offset, _)) = self.pending.first_key_value() else {
                break;
            };
            if offset != next_offset {
                break;
            }
            let chunk = self.pending.remove(&offset).expect("exists");
            self.ready.extend(chunk);
        }
    }

    fn read(&mut self, max_len: usize) -> Vec<u8> {
        let take = self.ready.len().min(max_len);
        let mut out = Vec::with_capacity(take);
        for _ in 0..take {
            if let Some(byte) = self.ready.pop_front() {
                out.push(byte);
            }
        }
        self.delivered_offset = self.delivered_offset.saturating_add(out.len() as u64);
        out
    }

    fn received_fin(&self) -> bool {
        self.final_offset
            .is_some_and(|offset| self.delivered_offset + self.ready.len() as u64 >= offset)
    }
}

/// Combined stream state machine.
#[derive(Debug)]
pub struct Stream {
    _id: StreamId,
    send: SendBuffer,
    recv: RecvBuffer,
}

impl Stream {
    fn new(id: StreamId) -> Self {
        trace!(stream = id.as_u64(), "creating stream");
        Self {
            _id: id,
            send: SendBuffer::default(),
            recv: RecvBuffer::default(),
        }
    }

    /// Queue application data for transmission.
    #[instrument(level = "trace", skip(self, data))]
    pub fn queue_send(&mut self, data: &[u8]) -> Result<(), StreamError> {
        self.send.queue(data)
    }

    /// Signal end-of-stream from local side.
    #[instrument(level = "trace", skip(self))]
    pub fn finish(&mut self) -> Result<(), StreamError> {
        self.send.queue_fin()
    }

    /// Fetch next chunk for transmission respecting `max_len`.
    #[instrument(level = "trace", skip(self))]
    pub fn next_send_chunk(&mut self, max_len: usize) -> Option<SendChunk> {
        self.send.next_chunk(max_len)
    }

    /// Write inbound data at a given offset.
    #[instrument(level = "trace", skip(self, data))]
    pub fn ingest(&mut self, offset: u64, data: &[u8], fin: bool) -> Result<(), StreamError> {
        self.recv.ingest(offset, data, fin)
    }

    /// Provide up to `max_len` bytes of contiguous received data to the caller.
    pub fn read(&mut self, max_len: usize) -> Vec<u8> {
        self.recv.read(max_len)
    }

    /// Determine whether the receive side reached EOF.
    #[must_use]
    pub fn is_receive_finished(&self) -> bool {
        self.recv.received_fin()
    }

    /// Check whether the send side has no pending data/FIN.
    #[must_use]
    pub fn is_send_drained(&self) -> bool {
        self.send.is_drained()
    }
}

/// Manager for all streams owned by an endpoint.
#[derive(Debug)]
pub struct StreamManager {
    _role: EndpointRole,
    streams: HashMap<StreamId, Stream>,
    flow: FlowController,
}

impl StreamManager {
    /// Create a new manager for the given endpoint role.
    #[must_use]
    pub fn new(role: EndpointRole) -> Self {
        Self {
            _role: role,
            streams: HashMap::new(),
            flow: FlowController::new(u64::MAX),
        }
    }

    /// Configure the connection-level send window (`MAX_DATA` from peer).
    pub fn set_connection_limit(&mut self, limit: u64) {
        self.flow.update_connection_limit(limit);
    }

    /// Configure a stream-specific send window (per-stream `MAX_DATA` from peer).
    pub fn set_stream_limit(&mut self, id: StreamId, limit: u64) {
        self.flow.update_stream_limit(id, limit);
    }

    /// Compute the remaining bytes that may be sent for the stream respecting connection limits.
    #[must_use]
    pub fn stream_send_allowance(&self, id: StreamId) -> u64 {
        self.flow.stream_available(id)
    }

    /// Obtain a mutable reference to a stream, creating it if required.
    pub fn get_or_create(&mut self, id: StreamId) -> &mut Stream {
        if !self.streams.contains_key(&id) {
            Metrics::record_stream_open();
        }
        self.streams.entry(id).or_insert_with(|| Stream::new(id))
    }

    /// Queue application data on a particular stream.
    #[instrument(level = "debug", skip(self, data))]
    pub fn queue_send(&mut self, id: StreamId, data: &[u8]) -> Result<(), StreamError> {
        self.streams
            .get_mut(&id)
            .ok_or(StreamError::UnknownStream)?
            .queue_send(data)
    }

    /// Queue a FIN marker on the stream.
    #[instrument(level = "debug", skip(self))]
    pub fn finish(&mut self, id: StreamId) -> Result<(), StreamError> {
        self.streams
            .get_mut(&id)
            .ok_or(StreamError::UnknownStream)?
            .finish()
    }

    /// Pull the next send chunk from a stream.
    pub fn poll_send_chunk(
        &mut self,
        id: StreamId,
        max_len: usize,
    ) -> Result<Option<SendChunk>, FlowControlError> {
        let allowance = self.flow.stream_available(id);
        if allowance == 0 {
            return Ok(None);
        }
        let limit = allowance
            .min(self.flow.connection_available())
            .min(max_len as u64) as usize;
        if limit == 0 {
            return Ok(None);
        }

        let Some(stream) = self.streams.get_mut(&id) else {
            return Ok(None);
        };

        let chunk = stream.next_send_chunk(limit);
        if let Some(ref chunk) = chunk {
            if !chunk.payload.is_empty() {
                self.flow.consume(id, chunk.payload.len() as u64)?;
            }
            debug!(
                stream = id.as_u64(),
                len = chunk.payload.len(),
                fin = chunk.fin,
                "emit stream chunk"
            );
        }
        Ok(chunk)
    }

    /// Ingest remote data for the specified stream.
    pub fn ingest(
        &mut self,
        id: StreamId,
        offset: u64,
        data: &[u8],
        fin: bool,
    ) -> Result<(), StreamError> {
        trace!(stream = id.as_u64(), offset, fin, "ingesting stream data");
        self.get_or_create(id).ingest(offset, data, fin)
    }

    /// Read fully contiguous data from the receive buffer.
    #[instrument(level = "trace", skip(self))]
    pub fn read(&mut self, id: StreamId, max_len: usize) -> Result<Vec<u8>, StreamError> {
        self.streams
            .get_mut(&id)
            .ok_or(StreamError::UnknownStream)
            .map(|stream| stream.read(max_len))
    }

    /// Check whether the stream send side is fully drained.
    pub fn is_send_drained(&self, id: StreamId) -> Result<bool, StreamError> {
        self.streams
            .get(&id)
            .ok_or(StreamError::UnknownStream)
            .map(Stream::is_send_drained)
    }

    /// Check whether the receive side observed FIN.
    pub fn is_receive_finished(&self, id: StreamId) -> Result<bool, StreamError> {
        self.streams
            .get(&id)
            .ok_or(StreamError::UnknownStream)
            .map(Stream::is_receive_finished)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stream_id_roundtrip() {
        let id = StreamId::new(EndpointRole::Client, StreamKind::Bidirectional, 42);
        assert_eq!(id.index(), 42);
        assert_eq!(id.role(), EndpointRole::Client);
        assert_eq!(id.kind(), StreamKind::Bidirectional);
        assert!(id.is_local_initiated(EndpointRole::Client));
        assert!(!id.is_local_initiated(EndpointRole::Server));

        let raw = id.as_u64();
        let parsed = StreamId::from_raw(raw);
        assert_eq!(parsed, id);
    }

    #[test]
    fn send_buffer_emits_chunks_and_fin() {
        let mut stream = Stream::new(StreamId::from_raw(0));
        stream.queue_send(b"hello").unwrap();
        stream.finish().unwrap();

        let chunk = stream.next_send_chunk(3).expect("chunk");
        assert_eq!(chunk.offset, 0);
        assert_eq!(chunk.payload, b"hel");
        assert!(!chunk.fin);

        let chunk = stream.next_send_chunk(8).expect("chunk");
        assert_eq!(chunk.offset, 3);
        assert_eq!(chunk.payload, b"lo");
        assert!(chunk.fin);

        assert!(stream.next_send_chunk(8).is_none());
        assert!(stream.is_send_drained());
    }

    #[test]
    fn recv_buffer_reassembles_and_detects_fin() {
        let mut stream = Stream::new(StreamId::from_raw(0));
        stream.ingest(2, b"llo", false).expect("ingest late chunk");
        stream.ingest(0, b"he", false).expect("ingest first chunk");
        stream.ingest(5, b"", true).expect("ingest fin");

        let data = stream.read(10);
        assert_eq!(data, b"hello");
        assert!(stream.is_receive_finished());
    }

    #[test]
    fn manager_queues_and_reads_streams() {
        let mut manager = StreamManager::new(EndpointRole::Client);
        let stream_id = StreamId::new(EndpointRole::Client, StreamKind::Bidirectional, 0);
        manager.get_or_create(stream_id); // create explicitly

        manager.queue_send(stream_id, b"abc").unwrap();
        let chunk = manager
            .poll_send_chunk(stream_id, 2)
            .unwrap()
            .expect("chunk");
        assert_eq!(chunk.payload, b"ab");
        assert!(!chunk.fin);

        manager.ingest(stream_id, 0, b"xyz", false).expect("ingest");
        let read = manager.read(stream_id, 8).unwrap();
        assert_eq!(read, b"xyz");
    }

    #[test]
    fn manager_respects_flow_limits() {
        let mut manager = StreamManager::new(EndpointRole::Client);
        let stream_id = StreamId::new(EndpointRole::Client, StreamKind::Bidirectional, 1);
        manager.get_or_create(stream_id);
        manager.set_connection_limit(5);
        manager.set_stream_limit(stream_id, 3);
        manager.queue_send(stream_id, b"abcdef").unwrap();

        let chunk = manager
            .poll_send_chunk(stream_id, 10)
            .unwrap()
            .expect("chunk");
        assert_eq!(chunk.payload, b"abc");
        assert_eq!(manager.stream_send_allowance(stream_id), 0);
        assert!(manager.poll_send_chunk(stream_id, 10).unwrap().is_none());
    }
}
