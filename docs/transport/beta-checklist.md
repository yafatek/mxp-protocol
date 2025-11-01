# MXP Transport Beta Rollout Checklist

## 1. Pre-Beta Gates

- ✅ Performance baseline recorded with `cargo run --example perf_baseline --release`; numbers logged in release tracker.
- ✅ Unit, integration (`tests/packet_engine.rs`, `tests/stream_flow.rs`), and fuzz suites green on main branch.
- ✅ Security review sign-off for Noise IK handshake + anti-amplification.
- ✅ Runbook (`docs/transport/runbook.md`) reviewed with operations and on-call teams.
- ✅ Feature flag / deployment toggle validated (ability to fall back to legacy QUIC path within 5 minutes).
- ✅ Observability dashboards exposing:
  - Active connections/streams
  - Flow-control consumption
  - Scheduler enqueue/dequeue rates
  - Packet loss / retransmit counts
  - Latency P50/P95/P99

## 2. Canary Deployment (Internal Clusters)

- Target scope: 5% of agent traffic in one region/AZ.
- Duration: Minimum 14 days.
- Monitoring thresholds (alert to on-call):
  - P99 latency > 400 µs for 5 minutes.
  - Packet loss (AckOutcome::lost > 2%) sustained for 5 minutes.
  - Connection error rate > 0.1%.
  - Replay/anti-amplification violations.
- Logging/Tracing: enable `info` for all transport modules, `debug` for handshake when investigating anomalies.
- Weekly report: include perf baseline delta, incident summary, tuning actions.

## 3. External Pilot (Customer Opt-In)

- Eligibility: customers with non-critical workloads who sign off on beta terms.
- Communication package:
  - Summary of benefits/perf targets.
  - Known limitations (lack of kernel-bypass, manual key rotation policy, etc.).
  - Support channel & SLA during beta.
- Monitoring thresholds tightened to customer SLOs; automatic rollback if any Sev2+ incident occurs.
- Capture customer feedback in shared beta log.

## 4. Rollback Plan

- Maintain legacy QUIC deployment artifacts for one release after GA.
- Rollback steps:
  1. Flip feature flag to disable MXP transport on affected clusters.
  2. Drain in-flight MXP connections gracefully (max 5 minutes) while accepting new QUIC sessions.
  3. Collect PCAP/metrics snapshots before/after for incident report.
- Post-rollback checklist: ensure runbook/doc updates, schedule root-cause analysis within 48 hours.

## 5. GA Gate Criteria

- All beta incidents resolved with documented fixes.
- Benchmarks meet or exceed targets in `docs/transport/targets.md` under production load.
- No Sev1/Sev2 issues for 30 consecutive days post-beta.
- Approval from Product, Security, and Operations leadership documented in release ticket.
