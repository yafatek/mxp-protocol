//! Zero-copy buffer pool for MXP transport packets.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Shared pool of reusable byte buffers.
#[derive(Clone, Debug)]
pub struct BufferPool {
    inner: Arc<PoolInner>,
}

#[derive(Debug)]
struct PoolInner {
    buffers: Mutex<VecDeque<Vec<u8>>>,
    buffer_size: usize,
    max_buffers: usize,
}

impl BufferPool {
    /// Create a new buffer pool.
    #[must_use]
    pub fn new(buffer_size: usize, max_buffers: usize) -> Self {
        assert!(buffer_size > 0, "buffer_size must be positive");
        assert!(max_buffers > 0, "max_buffers must be positive");

        let mut deque = VecDeque::with_capacity(max_buffers);
        for _ in 0..max_buffers {
            deque.push_back(vec![0u8; buffer_size]);
        }

        Self {
            inner: Arc::new(PoolInner {
                buffers: Mutex::new(deque),
                buffer_size,
                max_buffers,
            }),
        }
    }

    /// Acquire a buffer from the pool.
    pub fn acquire(&self) -> Buffer {
        let mut guard = self
            .inner
            .buffers
            .lock()
            .expect("buffer pool mutex poisoned");

        let buffer = guard
            .pop_front()
            .unwrap_or_else(|| vec![0u8; self.inner.buffer_size]);

        Buffer {
            data: Some(buffer),
            pool: Arc::clone(&self.inner),
            len: 0,
        }
    }

    /// Buffer capacity in bytes.
    #[must_use]
    pub fn buffer_size(&self) -> usize {
        self.inner.buffer_size
    }

    /// Maximum number of buffers managed by the pool.
    #[must_use]
    pub fn max_buffers(&self) -> usize {
        self.inner.max_buffers
    }
}

/// Buffer leased from the pool.
pub struct Buffer {
    data: Option<Vec<u8>>,
    pool: Arc<PoolInner>,
    len: usize,
}

impl Buffer {
    /// Reset the logical length of the buffer.
    pub fn reset(&mut self) {
        self.len = 0;
        if let Some(data) = self.data.as_mut() {
            data.fill(0);
        }
    }

    /// Expose the buffer as a mutable slice for writes.
    #[must_use]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        let data = self.data.as_mut().expect("buffer already returned to pool");
        &mut data[..]
    }

    /// Expose the filled portion of the buffer as an immutable slice.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        let data = self.data.as_ref().expect("buffer already returned to pool");
        &data[..self.len]
    }

    /// Current logical length of the buffer.
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Check whether the buffer contains no data.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Set the length of meaningful data within the buffer.
    pub fn set_len(&mut self, len: usize) {
        let capacity = self.capacity();
        assert!(len <= capacity, "buffer length exceeds capacity");
        self.len = len;
    }

    /// Return the configured capacity.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.data.as_ref().map_or(0, |data| data.len())
    }
}

impl Drop for Buffer {
    fn drop(&mut self) {
        if let Some(mut data) = self.data.take() {
            data.fill(0);
            let mut guard = self
                .pool
                .buffers
                .lock()
                .expect("buffer pool mutex poisoned");
            if guard.len() < self.pool.max_buffers {
                guard.push_back(data);
            }
        }
    }
}
