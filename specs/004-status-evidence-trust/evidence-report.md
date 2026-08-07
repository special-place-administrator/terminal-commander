# Evidence Report: 004 Status Evidence and Trust

Per `TESTING.md` §10. A report omitting source-status notes is incomplete and
the goal is not Completed.

## Branch

```text
004-status-evidence-trust
```

17 commits ahead of `main`. `git status --porcelain` clean at report time.

## What shipped

`command_status` no longer reports outcomes it cannot evidence.

| Story | Delivered |
|---|---|
| US1 | Restart-reconstructed outcomes carry the evidence a live observer had |
| US2 | Status is answered by the lane that OWNS the job, with real counters |
| US3 | `outcome_trust` on every response: observed / reconstructed / lost / abandoned |
| US4 | `JobLost` distinguishes a started-but-unfinished job from an unknown id |
| US5 | Abandonment recorded at graceful shutdown AND before stale replacement |
| US6 | Reconstruction reachable through the engine API, so both delivery modes agree |

## Files changed

47 files, +4430 / -195.

**Production** — `crates/store/migrations/V0008__outcome_evidence.sql` (new),
`crates/store/src/{job_receipt.rs,audit.rs}`,
`crates/ipc/src/{protocol.rs,lib.rs}`,
`crates/daemon/src/{command.rs,pty_command.rs,file_watch.rs,state.rs,runtime.rs,store_actor.rs,lib.rs}`,
`crates/daemon/src/ipc/{mod.rs,server.rs,handlers/command.rs}`,
`crates/mcp/src/tools.rs`,
`crates/supervisor/src/{ensure.rs,replace.rs}`.

**Tests (new)** — `status_lane_ownership.rs`, `status_reconstruction_evidence.rs`,
`status_lost_detection.rs`, `status_abandonment.rs`,
`windows_job_object_ownership.rs`, plus in-crate tests in
`daemon/src/ipc/handlers/command.rs` and `store/src/job_receipt.rs`.

**Gate** — `scripts/windows-gate.ps1` (tripwire registration).

**Docs** — `docs/mcp/OMNI_PLAYBOOK.md`, `BACKLOG.md` (TCD-8),
`docs/audits/…`, `docs/reviews/…`, `specs/004-*`, `.agent/goals/terminal-commander-status-trust/`.

## Verification commands

| Command | Result |
|---|---|
| `cargo fmt --all --check` | **PASS** (clean) |
| `cargo clippy --workspace --all-targets -- -D warnings` | **PASS** (exit 0) |
| `cargo nextest run --workspace --test-threads 4` | **PASS** — 926 passed, 1 skipped (was 910 at branch point) |
| `pwsh scripts/windows-gate.ps1` | **PASS** — `tc-gate: windows gate PASSED`, exit 0 |
| `pwsh scripts/linux-gate.ps1` (WSL) | **PASS** — 1200 passed, 1 skipped; `tc-gate: linux gate PASSED`, exit 0 |

Both OS gates were mandatory: this change touches `cfg(windows)` code **and**
adds tests (`CONTRIBUTING.md` §6.1).

### Test-parallelism finding (read before trusting a bare "green")

At **default** parallelism the full suite intermittently fails live-daemon CLI
tests (`read_subcommands`, `audit_subcommand`, `status_pid`) and
`pipe_client`'s retry test — **a different subset each run**.

Measured, not assumed:

| Run | Result |
|---|---|
| Full suite, default parallelism | 6 failures |
| Same tests isolated | 44/44 CLI pass |
| Full suite, `--test-threads 4` | **916/916 pass** |
| Full suite, `--test-threads 4` (later, more tests) | **926/926 pass** |

Windows named-pipe contention, not a regression. The six new
`DaemonState::bootstrap` tests added to the pressure. `windows-gate.ps1` selects
individual test binaries rather than running the whole suite, so it is not
exposed to this.

### Live dogfood

`CONTRIBUTING.md` §6.1 requires live dogfood with evidence. The defect was
originally measured against a live `0.1.86` daemon:

```text
pty_command_start ["hostname"]  -> job_019fd77e…c885
command_status     same job id  -> state exited, exit_code 0, duration_ms 813,
                                   frames_total 0, bytes_total 0, restarted false
command_output_tail same job id -> lines ["CRRR65734"], returned_lines 1
```

Same job: status claimed zero frames and zero bytes while the tail returned the
actual output line. `status_lane_ownership.rs` makes that combination
unreachable.

**VERIFIED post-fix (2026-08-07).** Re-run end-to-end through the real MCP stdio
adapter against a daemon built from this branch, on the same host, same command:

```text
pty_command_start ["hostname"] -> job_019fdbaf…4d1d
command_status                 -> state exited, exit_code 0, outcome_trust "observed",
                                  restarted false, frames_total 1, bytes_total 93,
                                  duration_ms 798
command_output_tail            -> lines ["CRRR65734"], returned_lines 1
```

Status and tail now agree. Pre-fix the same call reported `frames_total: 0`,
`bytes_total: 0` beside a tail holding the line.

Then the headline case — the daemon was hard-killed (`taskkill /F`, so the
in-memory job map is lost exactly as in the incident) and a cold adapter
re-bootstrapped a fresh daemon on the same data dir:

```text
command_status (same job) -> state exited, exit_code 0,
                             outcome_trust "reconstructed", restarted true,
                             frames_total 1, bytes_total 93, duration_ms 798,
                             probe_id prb_019fdbaf…e762
```

The evidence survived the restart **and matches the live observation exactly**
(1 / 93 / 798) — the R1 conjunction invariant holding in production, not only in
tests. Pre-fix this returned zeros and a null duration, which is what caused the
reporting session to discard a passing suite.

A well-formed but never-started id still returns `UnknownJob`
(`ipc_code: "UnknownJob"`), confirming it stays distinct from `JobLost`.

Cleanup: the branch daemons spawned for this probe were killed; the three
installed daemons were untouched.

## Source-status notes

| Behavior | Status | Note |
|---|---|---|
| V0008 migration + evidence persistence | `live` | Exercised by store round-trip tests and the reconstruction-equality test |
| Event-count conjunction (persist after append) | `live` | Pinned by asserting reconstructed `events_emitted` == live |
| Lane-routed status (`live_lane_status`) | `live` | Combed lane live; PTY/watch lanes proven by the ownership guard tests |
| PTY receipt persistence | `live` | Written by the PTY waiter; reconstruction path shared with the combed lane |
| `outcome_trust` wire field | `live` | Present on every response; compat asserted against the previous shape |
| `JobLost` detection | `live` | Predicate proven against a REAL started job, plus per-lane and decision-filter coverage |
| Graceful abandonment | `live` | Both shutdown arms; reconstruction reports `abandoned`, never `failed` |
| `QuiesceForReplace` | `partial` | Verb, dispatch and supervisor call are live and compile-verified; **no end-to-end test drives a real stale-daemon replacement**. The fallback path (timeout / unknown verb → proceed to kill) is by construction, not asserted. |
| Windows job-object ownership | `partial` | **Structural tripwire, not live fault injection.** Forcing `KILL_ON_JOB_CLOSE` mid-wait needs a test-only seam in production code, which constitution VI forbids; `CONTRIBUTING.md` §6.1 sanctions this fallback. Guards the three facts the ownership argument rests on. |
| ConPTY live child-output e2e | `blocked` | Pre-existing (O-07). Gate skips locally; CI runs it. Untouched by this work. |

No behavior is labelled `unknown` (a hard fail at commit per constitution VI).

## Risk register

Mitigates the honest-degradation class tracked as constitution VII compliance.
No existing RISK_REGISTER row is closed by this work. One new deferral recorded:

- **BACKLOG TCD-8** — `CommandRuntime.live` is never evicted. Deliberately not
  fixed here: the lingering binding is load-bearing (`status()` reads its
  terminal metrics aggregate; `collect_probes` reports terminal state from it),
  so eviction needs a replacement source for both first. Slow leak, not a
  correctness defect.

## Scope exclusions (decisions, not deferrals)

- **D2 — durable raw-tail persistence excluded.** Constitution III bars raw
  frames from persistent output. Evidence also shows it would not have helped:
  the tail was null even live in both reported incidents.
- **Proposal 2 merged into US1**, not dropped.

Scope cuts proposed during adversarial review (dropping the trust taxonomy,
lost-detection and the quiesce verb) were **rejected** by operator directive:
a deferral is acceptable only when an item is genuinely not needed for the plan
to succeed, not when it merely saves work.

## Corrections made during implementation

Recorded because each would otherwise have shipped as a silent defect:

1. **`Router::job_start` is dead code.** The planned lost-detection predicate
   would have matched nothing forever. Real actions are `command_start`,
   `pty_command_start`, `shell_session_start`, `file_watch_start`.
2. **The event-count fix needs both halves.** Moving the persist alone is a
   no-op (the count travels by value); changing the value alone overcounts on a
   failed append.
3. **`Option<u64>` counters were compat-breaking** and were dropped.
4. **Rejecting PTY ids would have destroyed the only readback** for a finished
   PTY job's exit code. Replaced by routing.
5. **`"abandoned"` as a terminal label** would have been coerced to `Failed`.
   Replaced by `end_cause` + trust indicator (D1).
6. **`OutcomeTrust` was never exported** from the daemon crate root; local
   Windows clippy passed because the only consumer sat in a `cfg(unix)` block.
   The linux gate would have caught it — which is why §6.1 mandates both.
