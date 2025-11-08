# MXP Custom Transport Implementation Plan

## 1. Module Layout & Ownership

| Crate / Module | Scope | Owner (initial) | Notes |
|----------------|-------|-----------------|-------|
| `mxpnexus-transport-core` | Socket driver, packet structs, buffer pools, timers | Transport team | Native implementation |
| `mxpnexus-transport-crypto` | Noise handshake, key schedule, AEAD wrappers | Crypto specialist | Re-exports hardened primitives only |
| `mxpnexus-transport-reliability` | ACK ranges, loss detection, congestion control | Networking engineer | Configurable algorithms (BBR-like, CUBIC fallback) |
| `mxpnexus-transport-stream` | Reliable stream state machines and buffers | Transport team | Implemented in `stream.rs` |
| `mxpnexus-transport-flow` | Connection/stream flow-control accounting | Transport team | `flow.rs` (in progress) |
| `mxpnexus-transport-scheduler` | Stream/dgram queues, priority logic, flow control | Protocol engineer | Initial WFQ scheduler in `scheduler.rs`; integration ongoing |
| `mxpnexus-transport-api` | Public Rust API exposing connections/streams | SDK interface lead | Defines MXP-native API surface |
| `mxpnexus-transport-tests` | Shared fixtures, fuzz harnesses, integration harness | QA/Infra | Lives under `tests/` with custom runner |

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
- [x] Unit/property tests for handshake transcripts.
- [x] Fuzz harness for handshake message parser.

### Phase 2 — Packet Engine & Reliability (Week 5-7)
- [x] Packet number encryption, header protection.
- [x] ACK frame generation/parsing with selective ranges.
- [x] Loss detection timers & RTT sampling.
- [x] Congestion control module (BBR-inspired default).
- [x] Anti-amplification guardrails and rate limiting.
- [x] Integration tests simulating loss/reorder with mock sockets.

- [ ] Stream abstraction (open/data/fin) with priority metadata *(core send/receive buffering landed; priority tagging pending)*.
- [ ] Datagram path with rate limiter *(queue + amplification budget checks merged; transmitter wiring next)*.
- [ ] Flow control windows (connection + stream) *(FlowWindow/FlowController tracking added; integrated with stream chunk emission; control frames outstanding)*.
- [ ] Flow control windows (connection + stream).
- [ ] Weighted fair queue scheduler honoring MXP message classes *(scheduler module scaffolding added; stream/datagram wiring pending)*.
- [ ] Backpressure signals to MXP core API.
- [ ] Benchmarks for mixed workloads.

### Phase 4 — Observability & Tooling (Week 11-12)
- [ ] Tracing spans for connection lifecycle, retransmits, scheduler decisions *(initial spans/logs wired into transport, streams, scheduler, loss tracking)*.
- [ ] Metrics integration with existing `Metrics` subsystem *(connection/stream counters hooked; scheduler/flow counters exposed via snapshot)*.
- [ ] Optional pcap exporter guarded by debug-only build configuration *(debug-tools feature writes raw packets to PCAP when paths configured; runbook guidance pending).* 
- [ ] Runbook draft and troubleshooting section *(initial runbook committed; iterate with ops feedback).* 

### Phase 5 — Hardening & Rollout (Week 13-16)
- [ ] Full test matrix execution (unit, property, fuzz, integration, soak).
- [ ] Performance benchmarking vs targets (docs/transport/targets.md) *(initial baseline via `examples/perf_baseline.rs`; automated tracking pending).* 
- [ ] Security review + external audit scheduling.
- [ ] Beta deployment to internal clusters; monitor telemetry for 14 days *(checklist documented in `docs/transport/beta-checklist.md`).*
- [ ] Customer pilot (limited partners) with fallback plan *(see beta checklist for comms template).* 
- [ ] Decision gate for enabling custom transport by default.
- [ ] Restore clippy pedantic enforcement *(temporarily disabled in CI for release; track cleanup post-GA).* 

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

- **MXP Nexus platform team:** Align on deployment mechanics, rollout controls, metrics ingestion.
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

