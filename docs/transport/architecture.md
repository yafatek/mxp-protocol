# MXP Custom Transport Architecture

## 1. Scope & Objectives

- Replace the QUIC dependency with a purpose-built transport tuned for MXP agent workloads.
- Deliver deterministic low-latency, high-throughput message delivery with minimal CPU and memory overhead.
- Provide a verifiable security envelope (mutual auth, forward secrecy, replay protection) suitable for enterprise adoption.
- Maintain the zero-copy guarantees and 32-byte aligned MXP header semantics already defined in the protocol core.

## 2. System Overview

```
┌────────────┐   ┌─────────────────┐   ┌───────────────────┐   ┌─────────────────────┐
│ UDP Socket │→→│ Packet Engine    │→→│ Reliability Layer  │→→│ Stream Scheduler     │→→ MXP Core
└────────────┘   └─────────────────┘   └───────────────────┘   └─────────────────────┘
                     ↑                        ↑                         ↑
                     └────── Security & Handshake ────────┬─────────────┘
                                                         Metrics & Tracing
```

### Layers

1. **Socket Driver** – owns UDP sockets (IPv4/IPv6), batching, time-stamping, and zero-copy buffer pools (aligned to cache lines).
2. **Packet Engine** – frames outbound packets (connection ID, packet number, AEAD tag) and deframes inbound packets while validating authenticity.
3. **Security Plane** – performs Noise-based handshake, key derivation, replay protection, key rotation, and optional hardware attestation checks.
4. **Reliability Plane** – handles acknowledgements, loss detection, retransmission, congestion control, and anti-amplification guardrails.
5. **Stream Scheduler** – multiplexes reliable ordered streams and unreliable datagrams per MXP message type priorities, enforcing flow control per agent.
6. **Observability** – emits tracing spans, metrics, and health signals for platform monitoring.

## 3. Design Goals

| Goal | Target |
|------|--------|
| Throughput | ≥ 1,000,000 64-byte messages/sec per core |
| P99 hop latency (same AZ) | ≤ 300 µs |
| Encode/Decode | ≤ 500 ns per message |
| Memory footprint | ≤ 128 MB per 10k concurrent streams |
| Packet loss recovery | Tolerate 1% loss with adaptive retransmit |
| Security posture | Mutual auth, forward secrecy, replay defense, key rotation ≤ 60 s |

## 4. Packet Format (Preliminary)

```
0                   1                   2                   3
0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
| ConnID (64b) | PN Len | Flags |   Packet Number (variable)    |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
| Header Len | AEAD Nonce (96b) | Payload ...                   |
+---------------------------------------------------------------+
| Auth Tag (128b)                                                  |
+-----------------------------------------------------------------+
```

- **ConnID:** Stable identifier supporting rebinding/migration.
- **Packet Number:** Monotonic per connection, encrypted after handshake.
- **Flags:** Packet type (Initial, Handshake, 0-RTT, 1-RTT, Probe), reserved bits must be zero.
- **Payload:** Contains one or more MXP frames (see §5) encrypted with AEAD.
- **Auth Tag:** 128-bit tag (ChaCha20-Poly1305 or AES-GCM).

Open items: final PN encoding, optional GREASE fields for version negotiation.

## 5. Frame Types

Each packet payload carries frames aligned to 8 bytes to preserve zero-copy semantics when referencing MXP headers.

| Frame | Purpose |
|-------|---------|
| `STREAM_OPEN` | Opens a reliable stream with priority metadata |
| `STREAM_DATA` | Carries MXP message slices for a specific stream |
| `STREAM_FIN` | Closes stream gracefully |
| `DATAGRAM` | Single-shot unreliable MXP payload |
| `ACK` | Cumulative + selective ACK ranges |
| `CONTROL` | Keepalive, window updates, migration tokens |
| `CRYPTO` | Handshake transcripts (Initial/Handshake/1-RTT) |

## 6. Handshake & Session Lifecycle

- **Pattern:** Noise IK with static server key, optional client static; ephemeral X25519 key exchange.
- **Authentication:** Server proves identity via MXP certificate signed by Relay CA; client optional.
- **Session Keys:** Derived via HKDF → AEAD (ChaCha20-Poly1305 default, AES-GCM alternative).
- **Replay Defense:** Nonces + ticket binding to client IP/ConnID; anti-amplification limit before authentication.
- **0-RTT Resume:** Stateless tickets containing resumption secrets, bounded lifetime, replay window tracking.
- **Key Rotation:** Rekey triggered every N packets or T seconds using Key Phase bits.
- **Migration:** Connection ID rotation + validation tokens to permit IP/port changes.

State machines for `Initial`, `Handshake`, `Established`, `Draining`, and `Closed` are documented in Appendix A (to be added once diagrams are complete).

## 7. Reliability & Congestion Control

- **ACK Strategy:** Hybrid ack-eliciting signals with selective ACK ranges to detect loss quickly.
- **Loss Detection:** Packet-based timer with exponential backoff, leveraging RTT samples per path.
- **Congestion Control:** Initially BBR-inspired pacing with simplified gain cycle; fallback to CUBIC under high loss.
- **Flow Control:** Per-connection and per-stream byte windows; MXP message priorities influence scheduler fairness.
- **Anti-Amplification:** Pre-auth requests capped at 3× inbound bytes until peer validated.

Open questions: Should unreliable datagrams bypass congestion control? (Tentative: rate-limited but not retransmitted.)

## 8. Stream Scheduler & Priorities

- **Priority Bands:**
  1. `Critical`: Call/Response (`0x10`/`0x11`).
  2. `Control`: AgentRegister/Discover/Heartbeat (`0x01`–`0x03`).
  3. `Streaming`: StreamOpen/Chunk/Close (`0x20`–`0x22`).
  4. `Background`: Event, Ack, Error (`0x12`, `0xF0`, `0xF1`).

- Weighted fair queuing ensures latency-sensitive frames preempt bulk transfers.
- Backpressure propagated to MXP core when per-stream window exhausted.

## 9. Buffer & Memory Model

- Pre-allocated ring buffers per direction with 32-byte alignment.
- Scatter/gather I/O to avoid payload copies.
- Adaptive pool resizing with telemetry-driven tuning.
- Guard pages in debug builds to detect overflow.

## 10. Observability & Telemetry

- **Tracing:** Structured spans for connection lifecycle, handshake, stream operations, packet retransmit.
- **Metrics:**
  - Connection count, stream count, packet loss, retransmit rate.
  - Latency histograms (send/receive, per priority band).
  - Crypto/key rotation counters.
- **Diagnostics:** Transport-level pcap exporter (guarded by debug-only build flag) for on-prem debugging.

## 11. Implementation Plan (High-Level)

1. Build transport skeleton as the primary implementation.
2. Implement handshake (Noise IK) with test harness and simulated peers.
3. Add packet framing + encryption/decryption with vectorized AEAD.
4. Layer in ACK/loss detection, then congestion control.
5. Integrate stream scheduler and MXP message framing.
6. Wire observability hooks and metrics integration.
7. Run fuzz/property tests, soak tests, perf benchmarks.
8. Gate default switch to custom transport once targets met.

Detailed milestones and ownership to be tracked in `docs/transport/plan.md` (to be created).

## 12. Open Questions & Next Steps

- Finalize cipher suite list and compliance requirements (FIPS?).
- Determine hardware acceleration roadmap (AES-NI, io_uring, DPDK).
- Define behavior under severe packet reordering (≥ 5 out-of-order).
- Align handshake with Relay platform identity/attestation model.
- Coordinate with SDK for stream/dgram APIs.

Next deliverables:
1. Performance & security target document (TODO-2).
2. Implementation plan with milestones (TODO-3).
3. Test matrix and verification strategy (TODO-5).
4. Whitepaper outline bridging architecture to product narrative (TODO-4).


