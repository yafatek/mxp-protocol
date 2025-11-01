//! Session ticket issuance and resumption primitives for MXP transport.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, SystemTime};

/// Length of ticket identifiers in bytes.
pub const TICKET_ID_LEN: usize = 16;
/// Length of ticket secrets in bytes.
pub const TICKET_SECRET_LEN: usize = 32;

/// Session resumption ticket issued after successful handshakes.
#[derive(Debug, Clone)]
pub struct SessionTicket {
    id: [u8; TICKET_ID_LEN],
    secret: [u8; TICKET_SECRET_LEN],
    issued_at: SystemTime,
    expires_at: SystemTime,
}

impl SessionTicket {
    /// Create a new ticket from components.
    #[must_use]
    pub fn new(id: [u8; TICKET_ID_LEN], secret: [u8; TICKET_SECRET_LEN], ttl: Duration) -> Self {
        let issued_at = SystemTime::now();
        let expires_at = issued_at + ttl;
        Self {
            id,
            secret,
            issued_at,
            expires_at,
        }
    }

    /// Ticket identifier accessor.
    #[must_use]
    pub fn id(&self) -> &[u8; TICKET_ID_LEN] {
        &self.id
    }

    /// Ticket secret accessor.
    #[must_use]
    pub fn secret(&self) -> &[u8; TICKET_SECRET_LEN] {
        &self.secret
    }

    /// Issued-at timestamp accessor.
    #[must_use]
    pub fn issued_at(&self) -> SystemTime {
        self.issued_at
    }

    /// Expiration timestamp accessor.
    #[must_use]
    pub fn expires_at(&self) -> SystemTime {
        self.expires_at
    }

    /// Determine whether the ticket is still valid.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.expires_at > SystemTime::now()
    }
}

/// Manages issuance and storage of session tickets.
#[derive(Debug, Clone)]
pub struct SessionTicketManager {
    ttl: Duration,
    max_entries: usize,
    counter: u64,
    tickets: HashMap<[u8; TICKET_ID_LEN], SessionTicket>,
    order: VecDeque<[u8; TICKET_ID_LEN]>,
}

impl SessionTicketManager {
    /// Construct a manager with the provided TTL and capacity.
    #[must_use]
    pub fn new(ttl: Duration, max_entries: usize) -> Self {
        Self {
            ttl,
            max_entries: max_entries.max(1),
            counter: 0,
            tickets: HashMap::new(),
            order: VecDeque::new(),
        }
    }

    /// Issue a new ticket seeded by the provided chaining key.
    pub fn issue(&mut self, seed: &[u8]) -> SessionTicket {
        self.prune_expired();
        self.counter = self.counter.wrapping_add(1);

        let (id, secret) = self.derive_material(seed);
        let ticket = SessionTicket::new(id, secret, self.ttl);

        self.store(ticket.clone());
        ticket
    }

    /// Attempt to resume a session given an identifier and expected seed.
    #[must_use]
    pub fn resume(&mut self, id: &[u8], seed: &[u8]) -> Option<SessionTicket> {
        if id.len() != TICKET_ID_LEN {
            return None;
        }
        let mut id_array = [0u8; TICKET_ID_LEN];
        id_array.copy_from_slice(id);

        self.prune_expired();

        if let Some(ticket) = self.tickets.get(&id_array) {
            if ticket.is_valid() {
                let (_, expected_secret) = self.derive_material(seed);
                if ticket.secret() == &expected_secret {
                    return Some(ticket.clone());
                }
            }
        }

        None
    }

    fn derive_material(&self, seed: &[u8]) -> ([u8; TICKET_ID_LEN], [u8; TICKET_SECRET_LEN]) {
        let mut id = [0u8; TICKET_ID_LEN];
        let mut secret = [0u8; TICKET_SECRET_LEN];

        let counter_bytes = self.counter.to_le_bytes();
        for (idx, byte) in id.iter_mut().enumerate() {
            let seed_byte = seed[idx % seed.len()];
            let counter_byte = counter_bytes[idx % counter_bytes.len()];
            *byte = seed_byte ^ counter_byte.rotate_left((idx % 8) as u32);
        }

        for (idx, byte) in secret.iter_mut().enumerate() {
            let seed_byte = seed[idx % seed.len()];
            let id_byte = id[idx % TICKET_ID_LEN];
            *byte = seed_byte
                .wrapping_add(id_byte)
                .rotate_left(((idx & 7) + 1) as u32);
        }

        (id, secret)
    }

    fn store(&mut self, ticket: SessionTicket) {
        if self.order.len() >= self.max_entries {
            if let Some(oldest) = self.order.pop_front() {
                self.tickets.remove(&oldest);
            }
        }

        self.order.push_back(*ticket.id());
        self.tickets.insert(*ticket.id(), ticket);
    }

    fn prune_expired(&mut self) {
        while let Some(id) = self.order.front() {
            if let Some(ticket) = self.tickets.get(id) {
                if ticket.is_valid() {
                    break;
                }
            }
            let removed = self.order.pop_front().expect("entry available");
            self.tickets.remove(&removed);
        }
    }
}
