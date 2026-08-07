# Implementation Plan: Status Evidence and Trust

**Branch**: `004-status-evidence-trust` | **Date**: 2026-08-06 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/004-status-evidence-trust/spec.md`

## Summary

`command_status` currently returns true terminal outcomes whose supporting
evidence has been discarded, in a response shape indistinguishable from a
fully-observed result. Two paths produce it: restart reconstruction (evidence
never persisted) and cross-lane reads (counts zeroed while the response asserts
they were observed live).

The approach is to make the engine report only what it actually knows, and to
make *how* it knows a first-class, closed-enum field:

1. Route a status read to the runtime that owns the job, so PTY and watch lanes
   return real counters instead of zeros.
2. Persist the evidence a live observer would have had, so a reconstructed
   outcome is usable rather than merely labelled.
3. Carry provenance in one closed enum (`observed` / `reconstructed` / `lost` /
   `abandoned`), with the existing `restarted` bool retained as a compat alias.
4. Diagnose "started but never finished" from the durable start record that
   already exists, and record abandonment at planned-death time.

## Technical Context

**Language/Version**: Rust, edition 2024. Active pin 1.97.1 (`rust-toolchain.toml`); documented MSRV floor 1.92.0.

**Primary Dependencies**: rmcp 1.8.0 (stdio MCP), rusqlite (WAL), tokio, parking_lot. No new dependency is introduced by this feature.

**Storage**: SQLite in WAL mode behind the repository-owned manual migration runner and a single-writer store actor. Refinery is intentionally not linked.

**Testing**: `cargo-nextest` for unit + integration, `cargo test --doc` for doctests. PR gate is `scripts/linux-gate.sh` and `scripts/windows-gate.ps1`, which CI invokes directly.

**Target Platform**: Windows 11 (named pipe IPC, ConPTY), Linux/WSL and macOS (UDS, POSIX PTY). This feature touches lane-generic code plus one Windows-specific regression guard.

**Project Type**: Rust workspace — privileged daemon (`terminal-commanderd`), stdio MCP adapter (`terminal-commander-mcp`), admin CLI (`terminal-commander`), plus supporting crates (`core`, `ipc`, `store`, `probes`, `sifters`, `supervisor`).

**Performance Goals**: No throughput change intended. A status read must remain a bounded, non-blocking operation; the new lost-detection lookup must not scan unboundedly as the audit table grows.

**Constraints**:
- Wire changes MUST be additive only. Mixed-version pairing is a standing configuration because stale-replacement replaces only *older* engines, so an older client against a newer engine is normal.
- No new network surface, no unbounded output, no raw frames into persistent storage.
- Existing readable outcomes MUST NOT become unreadable.

**Scale/Scope**: Roughly 8 production areas across 5 crates, one additive migration, one new IPC verb, six new tests, plus doc-contract corrections. No public tool is added or removed, so the MCP tool-count anchors are untouched.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Evaluated against `.specify/memory/constitution.md` v2.1.0.

| Principle | Verdict | Reasoning |
|---|---|---|
| **I. One Engine Boundary, Two Delivery Modes** (NON-NEGOTIABLE) | **PASS — and corrective** | Reconstruction currently lives in the IPC handler, so the documented embed surface cannot reach it. Moving it into the engine's own typed status API makes both delivery shapes terminate at the same authority. No second runner, policy engine, audit path, or storage authority is introduced. The adapter gains no engine logic. |
| **II. Policy-Before-Spawn, Default-Deny** (NON-NEGOTIABLE) | **PASS with one gate to mirror** | No new LLM-facing capability. The one new IPC verb (`QuiesceForReplace`) is supervisor-to-daemon and spawns nothing; it MUST adopt exactly the same authorization posture as the existing `Shutdown` verb — see research R5. No `[policy.caps]` flag is added, because no new caller-reachable capability exists. |
| **III. Combed, Bounded Output** (NON-NEGOTIABLE) | **PASS — decision D2 is constitutionally required, not merely prudent** | The principle bars raw frames from entering "buckets, rings, tails, context, logs, audit, snapshots, or persistent output". Persisting the no-silence tail into SQLite would put raw frames into persistent output. D2 (do not persist) is therefore the compliant choice. Everything this feature persists is bounded numeric/identifier evidence, never frame text. |
| **IV. Local-Only Privilege Boundary** | **PASS** | No transport change. No listener added. `QuiesceForReplace` rides the existing local endpoint the replacement handshake already uses. |
| **V. Audit Every Gated Action** | **PASS** | This feature *reads* existing audit rows for lost-detection and writes no new audit subject. No new secret-shaped value reaches an audit record. |
| **VI. No-Mock Production Paths and Verification Gate** (NON-NEGOTIABLE) | **PASS, with an explicit risk** | Every change is exercised by a real daemon test. One planned test (Windows job-object close) may require a fault-injection seam; if injection cannot be done without production code reaching test-only logic, the test is replaced by a documented invariant plus a `windows-gate.ps1` tripwire, per `CONTRIBUTING.md` §6.1. Source-status labels are required on every touched behavior. |
| **VII. Honest Degradation** (governing principle for this feature) | **PASS — and this is the principle being restored** | The current behavior is precisely what VII forbids: a response that is not a strict superset of what is known, presenting unobserved zeros as observed. VII also *independently* condemns the remedy this plan rejected — "never a bare error that discards a live job" rules out answering a live PTY job with a bare `UnknownJob`. VII further requires "closed typed codes plus bounded numeric or safe-enum fields", which is why provenance ships as a closed enum rather than free text. |

**Gate result: PASS.** No violation requires justification, so Complexity Tracking is empty.

**Non-negotiables re-checked post-design**: I, II, III, VI all still pass — the design adds no second engine path, no ungated capability, no raw frame persistence, and no production reach into test-only logic.

## Project Structure

### Documentation (this feature)

```text
specs/004-status-evidence-trust/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
│   ├── ipc-wire.md
│   └── mcp-facade.md
├── checklists/
│   └── requirements.md
├── spec.md
└── tasks.md             # Phase 2 output (/speckit-tasks)
```

### Source Code (repository root)

```text
crates/
├── core/src/job.rs                     # lifecycle vocabulary (READ ONLY - D1 adds no variant)
├── ipc/src/protocol.rs                 # response shape: trust enum, compat alias, doc contracts
├── store/
│   ├── migrations/V0008__*.sql         # additive: outcome evidence + audit subject index
│   └── src/{job_receipt.rs,audit.rs}   # evidence read/write, subject-filtered audit read
├── daemon/src/
│   ├── command.rs                      # persist-site reorder + evidence capture + owning-lane guard
│   ├── pty_command.rs                  # lane-owned status answer
│   ├── file_watch.rs                   # lane-owned status answer
│   ├── ipc/handlers/command.rs         # handler becomes pass-through to engine
│   ├── ipc/server.rs                   # QuiesceForReplace dispatch
│   └── runtime.rs                      # graceful-shutdown abandonment records
├── supervisor/src/replace.rs           # pre-kill quiesce handshake
└── mcp/src/tools.rs                    # agent-facing contract text + remedy text + pinned test

scripts/windows-gate.ps1                # Windows cfg tripwire registration
```

**Structure Decision**: This is an existing Rust workspace; no new crate is created. Changes are confined to the eight areas listed above. The lane-routing change is deliberately placed in the daemon engine rather than the IPC handler so the embedded delivery mode inherits it (Principle I).

## Complexity Tracking

> No Constitution Check violations. This table is intentionally empty.
