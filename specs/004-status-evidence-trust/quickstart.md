# Quickstart: validating Status Evidence and Trust

Runnable scenarios that prove the feature works end to end. Details of the wire
shape live in [contracts/ipc-wire.md](./contracts/ipc-wire.md); persisted shapes
live in [data-model.md](./data-model.md).

## Prerequisites

- Rust toolchain `1.97.1` (selected automatically by `rust-toolchain.toml`)
- `cargo-nextest`
- For the Linux gate on Windows: WSL provisioned per `CONTRIBUTING.md` §6.1
  (rustup + 1.97.1, `cargo-nextest`, `node`, `python3`)

## Verification gate

Constitution VI minimum for any Rust change. Route every build and test through
Terminal Commander's `run_and_watch` so failures and the real exit state are
preserved:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
```

This feature touches `cfg(windows)` code and adds tests, so **both** OS gates are
mandatory before push (`CONTRIBUTING.md` §6.1):

```bash
pwsh scripts/linux-gate.ps1     # runs the linux gate inside WSL
pwsh scripts/windows-gate.ps1   # windows-only regression gate
```

## Scenario 1 — A reconstructed pass carries usable evidence (US1)

The scenario that maps to the reported 40-minute loss.

1. Start a command that produces measurable output and runs long enough to have a
   duration.
2. Poll its status while the daemon is live. Record `frames_total`,
   `bytes_total`, `duration_ms`, `probe_id`, `events_emitted`.
3. Shut the daemon down gracefully; restart on the same data directory.
4. Poll the same job id again.

**Expected**: the second response reports the same output volume, duration and
originating probe as the first; `outcome_trust` is `reconstructed`; `restarted`
is `true`; `exit_code` is the real code, not null.

**Pin the off-by-one**: `events_emitted` must match the live value **including**
the lifecycle event. Scope this assertion to the natural-exit path only — on the
operator-stop path the persisted count can legitimately exceed the last live
value, because `stop()` publishes a snapshot before cancel while the persist uses
the reap-time count.

## Scenario 2 — Status never reports counts it did not observe (US2)

The defect that is live today, with no restart precondition.

1. Start an interactive (PTY) job that prints something.
2. Read its output tail; note the lines returned.
3. Poll `command_status` for the same job id.

**Expected**: reported output volume is non-zero and consistent with the tail;
`outcome_trust` is `observed`; state, exit code and duration remain readable.

**Regression form**: before this feature, status returned `frames_total: 0`,
`bytes_total: 0` and `restarted: false` while the tail returned real lines. That
combination must become unreachable.

Repeat for a file-watch job.

## Scenario 3 — The four provenance values are each reachable (US3)

Produce each condition and assert a distinct answer:

| Condition | Expected |
|---|---|
| Job observed end to end | `outcome_trust: observed` |
| Completed, then daemon restarted | `outcome_trust: reconstructed`, `restarted: true` |
| Started, daemon killed abruptly, restarted | `JobLost` |
| In flight during graceful shutdown | `outcome_trust: abandoned`, state cancelled, no exit code |
| Fabricated identifier | `UnknownJob` |

**Compatibility check**: decode every response above with a consumer built
against the previous shape. All must decode; `restarted` must retain its original
meaning. Zero decode failures is the pass condition — this is the check that
would have caught the rejected optional-counters design.

## Scenario 4 — Lost is distinguishable from unknown (US4)

1. Start a long-running command.
2. Kill the daemon **process** (not a graceful shutdown), so no receipt is
   written.
3. Restart and poll that job id.
4. Poll a fabricated job id.

**Expected**: (3) yields `JobLost`; (4) yields `UnknownJob`. Neither ever yields a
successful terminal outcome.

**Lane coverage**: repeat for a PTY job. Its start record uses a different audit
action, so a combed-only predicate would misclassify it as unknown.

## Scenario 5 — Planned death is recorded (US5)

**Graceful**: start a long job, trigger shutdown, restart, poll. Expect
`abandoned`, state cancelled, no exit code — and specifically **not** `failed`.

**Stale replacement**: start a long job, trigger a stale-daemon replacement,
restart, poll. Expect `abandoned` rather than a bare unknown identifier.

**Fallback**: with the quiesce verb unavailable or timing out, the replacement
must still proceed. Quiescing may never block an upgrade.

## Scenario 6 — Both delivery modes agree (US6)

Exercise the same reconstruction through the embedded engine surface
(`docs/EMBEDDING.md`) and confirm it returns the same answer as the adapter path.
Before this feature the embedded surface could not reach reconstruction at all.

## Manual dogfood

`CONTRIBUTING.md` §6.1 requires live dogfood on the affected OS with evidence.
The fastest end-to-end check is:

```bash
bash scripts/smoke/verify-runtime-smoke.sh
```

Then reproduce Scenario 2 by hand against a live daemon — start a PTY job, read
its tail, poll its status, and confirm the numbers agree. That is the exact
comparison that exposed the defect.

## Evidence to record

Per `TESTING.md` §10 every goal report must carry: the branch name, files
changed, PASS/FAIL per verification command, source-status labels for every
behavior touched, and any risk-register row this mitigates. A report missing
source-status labels is incomplete and the goal is not Completed.
