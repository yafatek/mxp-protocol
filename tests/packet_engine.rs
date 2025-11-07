use std::collections::{HashMap, VecDeque};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use mxp::transport::{
    AEAD_KEY_LEN, AEAD_TAG_LEN, AckFrame, AeadKey, AmplificationConfig, AntiAmplificationGuard,
    CongestionConfig, CongestionController, DEFAULT_MAX_ACK_RANGES, HEADER_PROTECTION_KEY_LEN,
    HEADER_SIZE, HeaderProtectionKey, LossConfig, LossManager, PacketCipher, PacketFlags,
    ReceiveHistory, SessionKeys, TransportError,
};

#[derive(Default)]
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        const A: u64 = 6364136223846793005;
        const C: u64 = 1442695040888963407;
        self.0 = self.0.wrapping_mul(A).wrapping_add(C);
        self.0
    }
}

struct SimPacket {
    to: usize,
    bytes: Vec<u8>,
    deliver_at: SystemTime,
}

struct SimLink {
    in_flight: Vec<SimPacket>,
    rng: Lcg,
    drop_rate: u64,
    delay_steps: u64,
    step_duration: Duration,
}

impl SimLink {
    fn new(seed: u64, drop_rate: u64, delay_steps: u64, step_duration: Duration) -> Self {
        Self {
            in_flight: Vec::new(),
            rng: Lcg(seed),
            drop_rate,
            delay_steps,
            step_duration,
        }
    }

    fn send(&mut self, now: SystemTime, packet: SimPacket) {
        if self.rng.next() % 100 < self.drop_rate {
            return;
        }
        let jitter = (self.rng.next() % self.delay_steps.max(1)) + 1;
        let mut packet = packet;
        packet.deliver_at = now + self.step_duration * (jitter as u32);
        self.in_flight.push(packet);
    }

    fn deliver<F>(&mut self, now: SystemTime, mut handler: F)
    where
        F: FnMut(usize, Vec<u8>),
    {
        let mut ready = Vec::new();
        let mut remaining = Vec::new();
        for packet in self.in_flight.drain(..) {
            if packet.deliver_at <= now {
                ready.push(packet);
            } else {
                remaining.push(packet);
            }
        }
        self.in_flight = remaining;
        ready.sort_by_key(|_| self.rng.next());
        for packet in ready {
            handler(packet.to, packet.bytes);
        }
    }
}

#[derive(Clone)]
struct OutboundPacket {
    payload: Vec<u8>,
    ack_eliciting: bool,
}

struct Endpoint {
    cipher: PacketCipher,
    recv_history: ReceiveHistory,
    loss: LossManager,
    cc: CongestionController,
    amp: AntiAmplificationGuard,
    outbound: VecDeque<OutboundPacket>,
    outstanding: HashMap<u64, OutboundPacket>,
    received: Vec<Vec<u8>>,
    conn_id: u64,
}

impl Endpoint {
    fn new(keys: SessionKeys, conn_id: u64) -> Self {
        let mut amp = AntiAmplificationGuard::new(AmplificationConfig::default());
        amp.mark_verified();
        Self {
            cipher: PacketCipher::new(keys),
            recv_history: ReceiveHistory::new(DEFAULT_MAX_ACK_RANGES, Duration::from_millis(0)),
            loss: LossManager::new(LossConfig::default()),
            cc: CongestionController::new(CongestionConfig::default()),
            amp,
            outbound: VecDeque::new(),
            outstanding: HashMap::new(),
            received: Vec::new(),
            conn_id,
        }
    }

    fn enqueue_message(&mut self, data: Vec<u8>) {
        self.outbound.push_back(OutboundPacket {
            payload: into_data_payload(data),
            ack_eliciting: true,
        });
    }

    fn on_receive(&mut self, now: SystemTime, bytes: Vec<u8>) -> Option<OutboundPacket> {
        self.amp.on_receive(bytes.len());
        let packet = match self.cipher.open(&bytes) {
            Ok(packet) => packet,
            Err(TransportError::ReplayDetected { .. }) => return None,
            Err(err) => panic!("decrypt failure: {err}"),
        };
        let payload = packet.payload();
        match payload.first().copied() {
            Some(0) => self.handle_data(now, payload, packet.header().packet_number()),
            Some(1) => self.handle_ack(now, payload),
            _ => None,
        }
    }

    fn handle_data(
        &mut self,
        now: SystemTime,
        payload: &[u8],
        packet_number: u64,
    ) -> Option<OutboundPacket> {
        self.received.push(payload[1..].to_vec());
        let immediate = self.recv_history.record(packet_number, true, now);
        if immediate {
            if let Some(frame) = self.recv_history.build_frame(now).unwrap() {
                let mut ack_payload = vec![1u8];
                frame.encode(&mut ack_payload);
                return Some(OutboundPacket {
                    payload: ack_payload,
                    ack_eliciting: false,
                });
            }
        }
        None
    }

    fn handle_ack(&mut self, now: SystemTime, payload: &[u8]) -> Option<OutboundPacket> {
        let frame = AckFrame::decode(&payload[1..]).expect("ack decode");
        let outcome = self.loss.on_ack_frame(&frame, now);
        for acked in &outcome.acknowledged {
            self.outstanding.remove(&acked.packet_number());
        }
        for lost in &outcome.lost {
            if let Some(pkt) = self.outstanding.remove(&lost.packet_number()) {
                self.outbound.push_front(pkt.clone());
            }
        }
        self.cc.on_ack_outcome(&outcome, now);
        None
    }

    fn tick(&mut self, now: SystemTime, link: &mut SimLink, peer: usize) {
        let mut inflight: usize = self.loss.outstanding().map(|pkt| pkt.size()).sum();
        let window = self.cc.window();

        while let Some(packet) = self.outbound.front().cloned() {
            if packet.ack_eliciting && inflight >= window {
                break;
            }
            let send_len = packet.payload.len();
            if !self.amp.try_consume(send_len) {
                break;
            }

            let mut buffer = vec![0u8; HEADER_SIZE + send_len + AEAD_TAG_LEN];
            let flags = if packet.ack_eliciting {
                PacketFlags::from_bits(PacketFlags::ACK_ELICITING)
            } else {
                PacketFlags::from_bits(PacketFlags::ACK)
            };
            let (pn, len) = self
                .cipher
                .seal_into(self.conn_id, flags, &packet.payload, &mut buffer)
                .expect("seal");
            buffer.truncate(len);

            if packet.ack_eliciting {
                self.loss.on_packet_sent(pn, now, len, true);
                self.cc.on_packet_sent(len);
                inflight = inflight.saturating_add(len);
                if let Some(stored) = self.outstanding.insert(pn, packet.clone()) {
                    self.outbound.push_front(stored);
                }
            }

            self.outbound.pop_front();
            let sim_packet = SimPacket {
                to: peer,
                bytes: buffer,
                deliver_at: now,
            };
            link.send(now, sim_packet);

            if !packet.ack_eliciting {
                continue;
            }
        }
    }
}

fn into_data_payload(mut data: Vec<u8>) -> Vec<u8> {
    let mut payload = Vec::with_capacity(data.len() + 1);
    payload.push(0);
    payload.append(&mut data);
    payload
}

fn make_session_keys(send_key: u8, recv_key: u8, send_hp: u8, recv_hp: u8) -> SessionKeys {
    SessionKeys::new(
        AeadKey::from_array([send_key; AEAD_KEY_LEN]),
        AeadKey::from_array([recv_key; AEAD_KEY_LEN]),
        HeaderProtectionKey::from_array([send_hp; HEADER_PROTECTION_KEY_LEN]),
        HeaderProtectionKey::from_array([recv_hp; HEADER_PROTECTION_KEY_LEN]),
    )
}

#[test]
fn packet_engine_survives_loss_and_reorder() {
    let base_time = UNIX_EPOCH + Duration::from_secs(1_000); // deterministic baseline
    let mut link = SimLink::new(0xfeed_beef, 10, 3, Duration::from_millis(5));

    let client_keys = make_session_keys(0x11, 0x22, 0x33, 0x44);
    let server_keys = make_session_keys(0x22, 0x11, 0x44, 0x33);

    let mut client = Endpoint::new(client_keys, 0xAAAA);
    let mut server = Endpoint::new(server_keys, 0xBBBB);

    let messages: Vec<Vec<u8>> = vec![
        b"hello".to_vec(),
        b"from".to_vec(),
        b"the".to_vec(),
        b"packet".to_vec(),
        b"engine".to_vec(),
    ];
    for msg in &messages {
        client.enqueue_message(msg.clone());
    }

    let mut now = base_time;
    for _step in 0..200 {
        client.tick(now, &mut link, 1);
        server.tick(now, &mut link, 0);

        link.deliver(now, |idx, bytes| {
            if idx == 0 {
                if let Some(ack_pkt) = client.on_receive(now, bytes) {
                    client.outbound.push_back(ack_pkt);
                }
            } else if let Some(ack_pkt) = server.on_receive(now, bytes) {
                server.outbound.push_back(ack_pkt);
            }
        });

        if let Some(deadline) = client.loss.loss_time() {
            if deadline <= now {
                let timed_out = client.loss.on_loss_timeout(now);
                for info in timed_out {
                    if let Some(pkt) = client.outstanding.remove(&info.packet_number()) {
                        client.outbound.push_front(pkt);
                    }
                }
            }
        }

        if server.received.len() == messages.len() && client.outbound.is_empty()
            && client.loss.outstanding().next().is_none() {
                break;
            }

        now += Duration::from_millis(5);
    }

    let mut received = server.received.clone();
    received.sort_by_key(|msg| {
        messages
            .iter()
            .position(|expected| expected == msg)
            .expect("known message")
    });
    received.dedup();
    assert_eq!(received, messages);
    assert!(client.loss.outstanding().next().is_none());
}
