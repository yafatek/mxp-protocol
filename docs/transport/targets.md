# MXP Custom Transport Targets

This document defines the measurable performance, security, and documentation objectives for the bespoke MXP transport. These targets guide implementation and serve as acceptance criteria before the new transport becomes the default carrier.

## 1. Performance Targets

| Metric | Target | Notes |
|--------|--------|-------|
| Sustained throughput | ≥ 1,000,000 64-byte messages/sec/core | Measured with single core, steady-state workload, no packet loss |
| Peak throughput | ≥ 5,000,000 64-byte messages/sec across 4 cores | Multi-core scaling benchmark |
| P50 encode/decode latency | ≤ 250 ns | Zero-copy encode/decode on modern x86/ARM |
| P99 encode/decode latency | ≤ 500 ns | Includes checksum, framing, AEAD overhead |
| P99 single-hop latency (same AZ) | ≤ 300 µs | Includes transport + MXP processing; measured end-to-end |
| P99 latency under 1% packet loss | ≤ 800 µs | With retransmission enabled |
| Connection setup time | ≤ 2 ms | Handshake RTT + crypto operations |
| Memory footprint per 10k streams | ≤ 128 MB | Includes buffers, state tables, key material |
| CPU utilization at 500k msg/sec | ≤ 40% of one core | Under typical payload mix |
| Packet amplification ratio | ≤ 1.1× after handshake | No more than 10% overhead from ACK/control packets |
| Backpressure response | ≤ 50 µs | Time to signal MXP core when flow control limits hit |

### Benchmark Scenarios

- **Hot path microbench:** encode/decode loops with varying payload sizes (64 B, 1 KB, 16 KB).
- **Lossy network simulation:** 0.1%, 1%, and 5% packet loss with 50 ms RTT.
- **Burst traffic:** 10× sustained rate for 100 ms to test buffer elasticity.
- **Long-lived streams:** 24-hour soak with periodic key rotations and migration events.
- **Mixed workload:** 50% Call/Response, 30% StreamChunk, 20% Events.

## 2. Security Targets

| Category | Requirement | Notes |
|----------|-------------|-------|
| Mutual authentication | Mandatory for server; optional client with policy control | Certificates issued by MXP Nexus CA or customer CA |
| Forward secrecy | X25519 ephemeral keys per session | Rekey every 10,000 packets or 60 seconds |
| Cipher suites | Default: X25519 + ChaCha20-Poly1305; Alt: P-256 + AES-256-GCM | Configurable via policy |
| Replay protection | Nonce tracking + ticket binding | 0-RTT tickets limited to 30 seconds |
| Key rotation | Bidirectional rekey with Key Phase bits | Graceful rekey without packet loss |
| Anti-amplification | Pre-auth limit ≤ 3× inbound bytes | Drop/tarpit after threshold |
| DoS resilience | Connection rate limiting + proof-of-work option | Configurable per deployment |
| Integrity | AEAD authenticated encryption over frames and header | Header protection prevents metadata tampering |
| Confidentiality | Mandatory AEAD encryption for all application payload | No plaintext control frames |
| Compliance readiness | FIPS 140-3 compatible cipher path | Toggleable to meet regulated workloads |
| Logging | Cryptographic events auditable without leaking secrets | Structured tracing with redaction |

### Security Validation Checklist

- Handshake formal review (state machine verification, transcript validation).
- Fuzzing of packet parser and handshake messages (libFuzzer/cargo-fuzz).
- Static analysis for constant-time operations on secret material.
- External security audit before GA.
- Replay attack simulation (delayed, duplicated Initial/Handshake packets).
- Pen-test of DoS limits (SYN flood equivalent, amplified datagrams).

## 3. Documentation Targets

1. **Architecture Doc** (complete): System overview, packet formats, state machines.
2. **Transport Targets Doc** (this file): Performance + security objectives.
3. **Implementation Plan** (`docs/transport/plan.md`): Milestones, module ownership, integration points.
4. **Test Plan** (`docs/transport/test-plan.md`): Coverage matrix for unit/property/fuzz/integration tests.
5. **Whitepaper Outline** (`docs/whitepaper/outline.md`): Public-facing narrative, threat model, benchmark strategy.
6. **API Spec** (`docs/transport/api.md`): Rust API and wire contracts for SDK integration.
7. **Operational Runbook** (`docs/transport/runbook.md`): Deployment guidance, tuning knobs, troubleshooting.

All documents must:
- Live in the repo alongside code, reviewed via PR.
- Include version headers and change logs.
- Cross-reference protocol spec (`SPEC.md`) and platform docs where relevant.
- Be kept in sync with implementation milestones.

## 4. Acceptance Criteria Before Default Switch

1. Performance benchmarks meet or exceed all targets in §1.
2. Security validation checklist completed with no critical findings.
3. Documentation suite up to date, including test evidence and operational guidance.
4. Integration tests with `mxpnexus` platform and `rust-deep-agents-sdk` green for 14 consecutive days.
5. Canary deployments in internal clusters for at least two weeks with telemetry review.
6. Rollback plan ready (document legacy QUIC branch for archival reference).

## 5. Ongoing Monitoring Targets

- Maintain P99 latency ≤ 400 µs in production (same AZ).
- Detect packet loss > 2% within 5 seconds and alert.
- Rotate keys automatically without exceeding 0.1% retransmit due to rekey.
- Ensure doc updates within 48 hours of any transport change.

These targets will be reviewed quarterly and adjusted as new hardware, workloads, and compliance requirements emerge.

