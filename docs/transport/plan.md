# MXP Custom Transport Implementation Plan

## 1. Module Layout & Ownership

| Crate / Module | Scope | Owner (initial) | Notes |
|----------------|-------|-----------------|-------|
| `relay-transport-core` | Socket driver, packet structs, buffer pools, timers | Transport team | Native implementation |
| `relay-transport-crypto` | Noise handshake, key schedule, AEAD wrappers | Crypto specialist | Re-exports hardened primitives only |
| `relay-transport-reliability` | ACK ranges, loss detection, congestion control | Networking engineer | Configurable algorithms (BBR-like, CUBIC fallback) |
| `relay-transport-scheduler` | Stream/dgram queues, priority logic, flow control | Protocol engineer | Hooks for MXP message priorities |
| `relay-transport-api` | Public Rust API exposing connections/streams | SDK interface lead | Defines MXP-native API surface |
| `relay-transport-tests` | Shared fixtures, fuzz harnesses, integration harness | QA/Infra | Lives under `tests/` with custom runner |

All crates live under `mxp-protocol/transport/` with internal dependencies only. No external runtime or benchmarking crates are permitted; bespoke tooling will be written in-house.

## 2. Build Configuration

- Custom transport is the only implementation shipped in this crate.
- No Cargo feature flags are used to toggle transport behaviour.
- Legacy QUIC carrier is removed; all tooling and CI run against the custom stack exclusively.

## 3. Milestones & Deliverables

### Phase 0 — Foundations (Week 0-1)
- [x] Scaffold crates and CI wiring.
- [x] Implement buffer pool abstraction with zero-copy slices.
- [x] Basic packet struct definitions (header, frame enums).
- [x] Doc updates (`architecture.md`, `plan.md`, API stubs).

### Phase 1 — Handshake & Security (Week 2-4)
- [x] Noise IK handshake implementation (client & server roles).
- [x] Key derivation + AEAD sealing/unsealing (ChaCha20-Poly1305 path).
- [x] Session ticket and anti-replay store.
- [ ] Unit/property tests for handshake transcripts.
- [ ] Fuzz harness for handshake message parser.

### Phase 2 — Packet Engine & Reliability (Week 5-7)
- [ ] Packet number encryption, header protection.
- [ ] ACK frame generation/parsing with selective ranges.
- [ ] Loss detection timers & RTT sampling.
- [ ] Congestion control module (BBR-inspired default).
- [ ] Anti-amplification guardrails and rate limiting.
- [ ] Integration tests simulating loss/reorder with mock sockets.

### Phase 3 — Streams & Scheduler (Week 8-10)
- [ ] Stream abstraction (open/data/fin) with priority metadata.
- [ ] Datagram path with rate limiter.
- [ ] Flow control windows (connection + stream).
- [ ] Weighted fair queue scheduler honoring MXP message classes.
- [ ] Backpressure signals to MXP core API.
- [ ] Benchmarks for mixed workloads.

### Phase 4 — Observability & Tooling (Week 11-12)
- [ ] Tracing spans for connection lifecycle, retransmits, scheduler decisions.
- [ ] Metrics integration with existing `Metrics` subsystem.
- [ ] Optional pcap exporter guarded by debug-only build configuration.
- [ ] Runbook draft and troubleshooting section.

### Phase 5 — Hardening & Rollout (Week 13-16)
- [ ] Full test matrix execution (unit, property, fuzz, integration, soak).
- [ ] Performance benchmarking vs targets (docs/transport/targets.md).
- [ ] Security review + external audit scheduling.
- [ ] Beta deployment to internal clusters; monitor telemetry for 14 days.
- [ ] Customer pilot (limited partners) with fallback plan.
- [ ] Decision gate for enabling custom transport by default.

## 4. Testing Alignment

Each milestone maps to specific entries in `docs/transport/test-plan.md` (to be created concurrently). CI gating:
- Unit tests run per crate.
- Fuzz jobs nightly.
- Performance benches in dedicated perf pipeline.
- Integration tests spun up with simulated networks (loss, latency, reorder).

## 5. Dependencies & Risks

- **Crypto primitives:** Evaluate `ring` vs `orion`; ensure FIPS-compliant alternative available.
- **Kernel bypass roadmap:** AF_XDP/DPDK integration deferred but APIs must remain abstracted.
- **Time sync:** RTT and loss detection rely on high-resolution timers; verify macOS/Linux parity.
- **Resource limits:** Pre-allocate buffers carefully to avoid memory spikes under load.
- **Compliance:** Engage security/legal early for cryptographic exports (if needed).

## 6. Coordination Points

- **Relay platform team:** Align on deployment mechanics, rollout controls, metrics ingestion.
- **rust-deep-agents-sdk:** Mirror API adjustments; provide compatibility shims.
- **DevOps/Infra:** Provision perf test clusters, network emulation environments, observability dashboards.
- **Security team:** Schedule audits, track fixes.

## 7. Communication & Checkpoints

- Bi-weekly transport syncs with recap of progress vs milestones.
- Living status doc referencing this plan and Jira/issue tracker items.
- Demo checkpoints at the end of each phase (handshake demo, stream scheduler demo, etc.).
- Publish updates to community roadmap once beta ready.

## 8. Exit Criteria

Custom transport considered production-ready when:
1. All milestones completed with documented evidence.
2. Performance and security targets met or exceeded.
3. Documentation suite finished (architecture, targets, test plan, runbook, whitepaper).
4. Successful beta with no Sev1/Sev2 incidents.
5. QUIC carrier maintained as fallback for one release post-GA.

This plan will be updated as dependencies shift or milestones are refined.

