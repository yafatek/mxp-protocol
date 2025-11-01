# MXP Custom Transport – Product Requirements Document (PRD)

## 1. Purpose
Deliver a bespoke transport layer tailored for MXP agent communication that:
- Achieves sub-millisecond latency and million-msg/sec throughput per connection.
- Provides enterprise-grade security (mutual auth, forward secrecy, replay defense).
- Integrates seamlessly with Relay platform and rust-deep-agents-sdk.
- Operates with zero external dependencies (pure Rust implementation).

## 2. Stakeholders
- **Product Lead:** Defines customer needs, prioritizes roadmap.
- **Transport Engineering:** Implements protocol stack, ensures performance.
- **Security Team:** Reviews cryptographic design, threat model, compliance.
- **Platform/DevOps:** Deploys, monitors, and operates transport in cloud/on-prem.
- **SDK Team:** Updates client APIs and tooling for agent developers.

## 3. Target Users & Use Cases
- Enterprise teams deploying collaborative AI agents across clouds and on-prem.
- High-frequency agent workflows requiring deterministic low latency.
- Regulated industries needing auditable, secure communication channels.
- Platform operators managing multi-tenant agent clusters.

### Key Scenarios
1. **Agent Mesh in Same Region:** Thousands of agents exchanging RPC calls with millisecond deadlines.
2. **Cross-Region Federation:** Agents collaborating across controlled networks with resilience to packet loss.
3. **Enterprise Onboarding:** Customer integrates MXP within existing network policy constraints.
4. **Observability-Driven Operations:** Operators monitor transport health, investigate anomalies quickly.

## 4. Requirements

### Functional Requirements
1. Establish secure connections using Noise IK handshake with mutual authentication.
2. Support both reliable streams and best-effort datagrams with priority scheduling.
3. Enforce flow control and congestion control tuned for agent workloads.
4. Provide telemetry hooks (metrics, tracing) for platform observability.
5. Allow hot key rotation, connection migration, and graceful drain.
6. Offer UDP carrier (default) with abstraction for future raw-IP/AF_XDP carriers.
7. Maintain compatibility with existing MXP message API.

### Non-Functional Requirements
1. Meet performance/security targets documented in `docs/transport/targets.md`.
2. Pure Rust implementation—no external dependency beyond standard library where achievable.
3. Deterministic resource usage with bounded memory per stream.
4. Robust error handling and recovery (timeouts, retransmit, backpressure).
5. Comprehensive documentation and test coverage (see test plan).

## 5. Constraints & Assumptions
- Initial carrier is UDP to ensure Internet compatibility.
- All cryptographic primitives implemented or wrapped in-house to avoid third-party crates.
- Deployment environments include AWS, GCP, Azure, and on-prem Linux/macOS hosts.
- QUIC remains available as fallback until custom transport reaches GA criteria.

## 6. Success Metrics
- Latency/throughput metrics as per performance targets.
- Security validation passes without critical findings.
- Operator feedback (pilot customers) confirms manageability and stability.
- Reduction in CPU/memory usage compared to QUIC baseline by ≥ 20%.
- No Sev1 incidents during beta and rollout.

## 7. Dependencies
- Architecture, target, plan, and test documents (already drafted).
- Internal tooling for fuzzing/perf (implemented in-house).
- Coordination with platform/SDK teams for rollout logistics.

## 8. Release Phases
1. **Alpha (internal):** Feature flag disabled by default; developers exercise API.
2. **Beta (selected partners):** Collect telemetry, iterate on tuning.
3. **GA:** Custom transport enabled by default with QUIC fallback for one release.
4. **Post-GA:** Explore raw-IP carrier, kernel bypass, and hardware acceleration.

### Current Status (2025-11-01)
- Phase 2 (Packet Engine & Reliability) deliverables are complete, including integration harness with loss/reorder simulation.
- Phase 3 kicked off: foundational stream send/receive buffering merged; datagram queue with amplification guard landed; flow-control bookkeeping in place (control frames pending); scheduler and prioritization remain in flight.

## 9. Risks & Mitigations
- **Performance shortfall:** Build perf harness early; optimize buffer management and schedulers.
- **Security gaps:** Engage security review early; run fuzz/property testing continuously.
- **Operational complexity:** Provide runbook, dashboards, and automated alerts.
- **Adoption friction:** Maintain compatibility APIs and clear migration guides.

## 10. Open Questions
- Final cipher suite list for compliance (FIPS mode requirements).
- Prioritization of kernel bypass roadmap vs. customer demand.
- SLA commitments for different deployment topologies.

This PRD will be updated as implementation progresses and customer feedback is gathered.

