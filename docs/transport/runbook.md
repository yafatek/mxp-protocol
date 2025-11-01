# MXP Transport Runbook

## 1. Operational Checklist

- **Build Flags**: Ensure release builds use `--features debug-tools` only in staging when packet capture is required; production builds stay feature-less.
- **Configuration**: Verify `TransportConfig` limits (buffers, timeouts, flow windows) match deployment sizing guidelines.
- **Metrics Export**: Confirm relay runtime scrapes MXP transport counters (connection/stream totals, flow, scheduler, datagram statistics) every 5 s.
- **Tracing Level**: Default to `info` in production; elevate to `debug` for targeted investigations.

## 2. Monitoring & Alerting

| Signal | Threshold | Action |
|--------|-----------|--------|
| `transport.active_connections` | Sudden drop > 20% | Check upstream load balancers and handshake errors |
| `transport.datagram_sent` vs `datagram_enqueued` | Stall > 60 s | Inspect amplification guard + flow control limits |
| `transport.flow_bytes_consumed` | Plateau under high load | Evaluate stream limits, backpressure to MXP core |
| P99 latency | > 400 µs sustained | Run perf baseline, inspect congestion/loss metrics |
| Packet loss (`AckOutcome::lost`) | > 2% | Validate network health, anti-amplification budget |

## 3. Debug Workflow

1. **Reproduce**: Capture packet traces using `TransportConfig::pcap_*_path` in staging; run `cargo run --example perf_baseline --release` to compare against golden metrics.
2. **Inspect Traces**: Use Wireshark/`tshark` with raw (encrypted) packets to verify timing, packet counts, amplification adherence.
3. **Correlate Metrics**: Review scheduler enqueue/dequeue deltas and flow-control counters for imbalances.
4. **Deep Dive**: Enable `debug` tracing for `mxp::transport::*`; review handshake and retransmission logs.
5. **Validate Fix**: Re-run perf baseline + targeted integration tests (`tests/packet_engine.rs`, `tests/stream_flow.rs`).

## 4. Incident Response

- **Severity Determination**: If agent traffic is degraded but functional (latency spike), classify as Sev2; complete outage is Sev1.
- **Immediate Steps**:
  - Capture current metrics snapshot.
  - Toggle fallback to legacy transport (QUIC) using deployment feature switch.
  - Notify on-call security if anomaly involves handshake/authentication.
- **Post-Mitigation**:
  - Attach PCAP logs, metrics diffs, perf baseline results to the incident ticket.
  - Update this runbook with remediation notes and prevention steps.

## 5. Maintenance

- **Quarterly**: Refresh perf baseline numbers, review targets in `docs/transport/targets.md`, and adjust thresholds.
- **Before Major Release**: Re-run full test matrix (`cargo test`, fuzz harnesses, packet engine simulation) and document findings in the release checklist.
- **Documentation Sync**: Update runbook within 48 hours of any transport change impacting operations.
