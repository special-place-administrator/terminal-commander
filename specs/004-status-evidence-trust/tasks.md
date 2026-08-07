---

description: "Task list for 004 Status Evidence and Trust"
---

# Tasks: Status Evidence and Trust

**Input**: Design documents from `/specs/004-status-evidence-trust/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: REQUIRED. Constitution VI (NON-NEGOTIABLE) forbids treating "it
compiled" as proof and mandates the verification gate; `CONTRIBUTING.md` §6.1
additionally requires a regression test for any platform-asymmetric daemon fix.

**Organization**: Grouped by user story so each is independently implementable
and testable.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency on incomplete work)
- **[Story]**: `[US1]`..`[US6]` maps to spec.md user stories

## Path Conventions

Existing Rust workspace. Crate paths are repository-relative, e.g.
`crates/daemon/src/command.rs`. No new crate is created.

---

## Phase 1: Setup

**Purpose**: Confirm the working environment matches the pinned toolchain.

- [ ] T001 Confirm the pinned toolchain resolves and the workspace builds clean from `rust-toolchain.toml` at repository root
- [ ] T002 [P] Confirm `cargo-nextest` is installed and `cargo nextest run --workspace` passes on the untouched branch, to establish a pre-existing-failure baseline for the evidence report

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Schema, storage API, and wire vocabulary every story depends on.

**⚠️ CRITICAL**: No user story work can begin until this phase completes.

- [ ] T003 Add `crates/store/migrations/V0008__outcome_evidence.sql` with three additive statements: `metrics_json TEXT` on `job_receipts`, `end_cause TEXT` on `job_receipts`, and `idx_audit_records_action_subject` on `audit_records(action, subject)`. No backfill, no drops.
- [ ] T004 Extend the receipt row struct and read/write in `crates/store/src/job_receipt.rs` to carry `metrics_json` and `end_cause`, both nullable. A row with `metrics_json` absent MUST remain valid and MUST NOT default to zeroes.
- [ ] T005 [P] Add a subject filter to `AuditReadRequest` and its store read in `crates/store/src/audit.rs`. Today only `action_filter` and `decision_filter` exist, so the lost-detection predicate cannot be expressed at all (research R2).
- [ ] T006 [P] Add the `OutcomeTrust` closed enum to `crates/ipc/src/protocol.rs` per `contracts/ipc-wire.md`: variants `observed`/`reconstructed`/`lost`/`abandoned`, `#[serde(rename_all = "snake_case")]`, `#[default] Observed`.
- [ ] T007 Add `outcome_trust` to `CommandStatusResponse` in `crates/ipc/src/protocol.rs` with `#[serde(default)]`. Leave the five counter fields as bare `u64` — optionality is rejected (research R4). Keep `restarted` and its meaning.
- [ ] T008 [P] Add the `JobLost` error code to the IPC error enum in `crates/ipc/src/protocol.rs` and map it in `crates/daemon/src/ipc/handlers/common.rs`.
- [ ] T009 Extend `StoreOp` in `crates/daemon/src/store_actor.rs` with the receipt-evidence write and the subject-filtered audit read, so all SQLite I/O stays on the single-writer actor thread.

**Checkpoint**: schema, storage API, and wire vocabulary exist. Stories may begin.

---

## Phase 3: User Story 2 - Status never reports counts it did not observe (Priority: P1) 🎯 MVP

**Goal**: A status read is answered by the runtime that owns the job, so PTY and
watch lanes return real counters instead of zeros asserted as observations.

**Why first**: live today with no precondition, and it blocks US3 — a trust
indicator reading `observed` on a cross-lane read would certify the lie.

**Independent Test**: start a PTY job that prints, poll its status, and confirm
the reported output volume matches what the output tail returns for that job.

### Tests for User Story 2

- [ ] T010 [P] [US2] Add a daemon regression test in `crates/daemon/tests/` asserting that `command_status` for a PTY job reports non-zero output volume matching the ring, and never `frames_total: 0` alongside `outcome_trust: observed`
- [ ] T011 [P] [US2] Extend that test to a file-watch job, whose lane has the same shared-ledger exposure

### Implementation for User Story 2

- [ ] T012 [US2] Add an ownership guard to `CommandRuntime::status` in `crates/daemon/src/command.rs` so it answers only for jobs it owns. State the chosen invariant in a comment — `live`-map presence mirrors `stop()` at `:1613-1614` and does not depend on the lifecycle enum, but is only safe while bindings are never evicted (research R3).
- [ ] T013 [US2] Add a lane-owned status answer to `crates/daemon/src/pty_command.rs`, sourcing real counters the way `PtyRuntime::list` already does at `:756-771`
- [ ] T014 [P] [US2] Add the equivalent lane-owned status answer to `crates/daemon/src/file_watch.rs`
- [ ] T015 [US2] Dispatch the status request to the owning lane in `crates/daemon/src/ipc/handlers/command.rs`. It MUST NOT answer a live job with a bare `UnknownJob` — Constitution VII forbids "a bare error that discards a live job".
- [ ] T016 [US2] Re-read the three internal `status()` callers against the widened behaviour: `crates/daemon/src/ipc/server.rs:1045`, `crates/daemon/src/ipc/handlers/runtime.rs:17-18`, `crates/daemon/src/subscriptions/pull.rs:428-429`. All were verified unable to pass a cross-lane id; confirm that still holds.
- [ ] T017 [US2] Verify `command_output_tail` still answers cross-lane and still returns real frames. It is correct today and must not be swept up by the guard.

**Checkpoint**: no lane reports zero counters while output exists.

---

## Phase 4: User Story 1 - A reconstructed pass carries usable evidence (Priority: P1) 🎯 MVP

**Goal**: A restart-reconstructed outcome carries the evidence a live observer
would have had, so an agent banks the pass instead of re-running.

**Independent Test**: run a command to completion, record the live status,
restart, poll again, and confirm the evidence matches.

### Tests for User Story 1

- [ ] T018 [P] [US1] Add a test asserting reconstructed counters, `duration_ms`, and `probe_id` match the pre-restart live status. Scope the `events_emitted` equality to the **natural-exit** path only — on the `stop()` path the persisted count can legitimately exceed the last live value (research R8).
- [ ] T019 [P] [US1] Add a test asserting a pre-migration receipt row (no `metrics_json`) is reported honestly and never as zeroes-presented-as-observed
- [ ] T020 [P] [US1] Add a test asserting that after a restart the absent no-silence tail does not read as "the command produced no output" (decision D2)

### Implementation for User Story 1

- [ ] T021 [US1] Capture evidence at the terminal transition in `crates/daemon/src/command.rs` — frames, bytes, suppression counts, `duration_ms` from the job record's exit info, and the binding's `probe_id` — and pass it to the receipt write
- [ ] T022 [US1] Apply the event-count conjunction in `crates/daemon/src/command.rs`: move the natural-exit persist below the lifecycle-append block and pass the post-append `final_metrics.events_emitted`. Leave `rule_driven_events` untouched so the no-silence gate at `:1403` still sees the pre-bump value. Do **not** compute `+1` inline at the current site — it sits outside the `is_ok()` guard and would overcount when the append fails (research R1).
- [ ] T023 [US1] Leave the early-return persist at `:1457` on the reap-time count. `stop()` performs no append (`:1639`), so that site must not add one.
- [ ] T024 [US1] Populate the reconstruction from persisted evidence instead of hardcoded zeros, in `crates/daemon/src/ipc/handlers/command.rs`. Keep the real `exit_code` — never force null (spec FR-008).
- [ ] T025 [US1] Persist a receipt from the PTY waiter in `crates/daemon/src/pty_command.rs`, mirroring the combed finalization. Metrics and identifiers only — no frame text, per Constitution III and decision D2.

**Checkpoint**: a reconstructed pass is usable, not merely labelled.

---

## Phase 5: User Story 3 - Provenance is one obvious field (Priority: P2)

**Goal**: Every response carries a closed-enum trust indicator, and agents are
told what each value means.

**Depends on**: US2 (otherwise `observed` certifies a falsehood).

**Independent Test**: produce each condition and confirm a distinct documented answer.

### Tests for User Story 3

- [ ] T026 [P] [US3] Add a test asserting `outcome_trust` is present on **every** status response, mirroring `run_and_watch_normal_terminal_is_complete_and_a_strict_superset` (`crates/mcp/src/tools.rs:6345`)
- [ ] T027 [P] [US3] Add a wire-compatibility test decoding new responses against the previous struct shape, asserting zero decode failures and that `restarted` retains its meaning

### Implementation for User Story 3

- [ ] T028 [US3] Set `outcome_trust` at every construction site of `CommandStatusResponse`, deriving `restarted` from it rather than setting the two independently
- [ ] T029 [US3] Pass `outcome_trust` through `command_status_payload` in `crates/mcp/src/tools.rs:4894`, and correct the now-false `//` comment at `:4914-4918`
- [ ] T030 [US3] Rewrite the `command_status` tool description at `crates/mcp/src/tools.rs:1525` per `contracts/mcp-facade.md`. It currently never mentions `restarted`, so this writes a contract where none exists. Include the sentence that a missing tail does not mean no output.
- [ ] T031 [P] [US3] Add `outcome_trust` to the `run_and_watch` description at `crates/mcp/src/tools.rs:1293`, alongside the existing `degraded`/`recover_hint` vocabulary
- [ ] T032 [P] [US3] Update `docs/mcp/OMNI_PLAYBOOK.md` so the interactive/REPL section states a finished PTY job's status is readable and its counters are real

**Checkpoint**: an agent branches on one field instead of inferring from zeros.

---

## Phase 6: User Story 4 - Lost is distinguishable from unknown (Priority: P2)

**Goal**: A job the engine recorded starting but never recorded finishing is
diagnosed, not misreported as an unrecognised identifier.

### Tests for User Story 4

- [ ] T033 [P] [US4] Add a test: start a job, kill the daemon **process** (not graceful), restart, assert `JobLost` and never a successful terminal outcome
- [ ] T034 [P] [US4] Add a test asserting a fabricated identifier still yields `UnknownJob`
- [ ] T035 [P] [US4] Add a PTY-lane case, since its start record uses a different audit action and a combed-only predicate would misclassify it

### Implementation for User Story 4

- [ ] T036 [US4] Implement lost-detection on the not-found fallback in `crates/daemon/src/ipc/handlers/command.rs`, predicating on `action IN ('command_start','pty_command_start','file_watch_start') AND subject = <job id> AND decision = 'allow'`. Do **not** use `job_start` — `Router::job_start` is dead in production (research R2).
- [ ] T037 [US4] Document in code that detection is best-effort because audit emits are swallowed (`crates/daemon/src/command.rs:787`), and that an inconclusive lookup MUST degrade to `unknown`, never to a terminal outcome

**Checkpoint**: three cases separable; failure direction is safe.

---

## Phase 7: User Story 5 - Planned death is recorded (Priority: P3)

**Goal**: Shutdown and stale-replacement record abandonment instead of leaving
silence.

**Depends on**: decision D1 — abandonment rides the trust indicator; the lifecycle
state stays `cancelled`. Writing an unrecognised terminal label would surface as
`failed`, creating exactly the false negative this feature removes.

### Tests for User Story 5

- [ ] T038 [P] [US5] Add a test: in-flight job plus graceful shutdown ⇒ post-restart `outcome_trust: abandoned`, state cancelled, no exit code, and specifically **not** failed
- [ ] T039 [P] [US5] Add a test: stale-replacement path ⇒ abandonment recorded rather than a bare unknown identifier
- [ ] T040 [P] [US5] Add a test asserting that a quiesce timeout or an old daemon that does not know the verb still lets the replacement proceed

### Implementation for User Story 5

- [ ] T041 [US5] Write abandonment records for non-terminal jobs during the lifecycle drain in `crates/daemon/src/runtime.rs`, before `shutdown_store`. The drain already precedes store close on both arms (`:376-377` unix, `:452-453` windows).
- [ ] T042 [US5] Add the `QuiesceForReplace` request/response to `crates/ipc/src/protocol.rs` and dispatch it in `crates/daemon/src/ipc/server.rs`, mirroring `Shutdown`'s ungated posture exactly (research R5)
- [ ] T043 [US5] Call the quiesce verb before `hard_kill` in `crates/supervisor/src/replace.rs`, bounded, with fallback to current behaviour on timeout or error. Quiescing MUST NEVER block a replacement.
- [ ] T044 [US5] Map `end_cause = abandoned` to `outcome_trust: abandoned` in the reconstruction, with lifecycle state `cancelled` and null exit code

**Checkpoint**: planned deaths are accounted for; unplanned ones fall to US4.

---

## Phase 8: User Story 6 - Both delivery modes agree (Priority: P3)

**Goal**: The embedded engine surface returns the same answers as the adapter.

- [ ] T045 [P] [US6] Add a test exercising reconstruction through the engine's typed status API directly, asserting parity with the adapter path
- [ ] T046 [US6] Move reconstruction from the IPC handler into `CommandRuntime::status` in `crates/daemon/src/command.rs`, leaving the handler a pass-through. Document explicitly that `status()` now performs a write (`mark_job_receipt_restarted`) and that its not-found contract changed for all three internal callers (research R8).

**Checkpoint**: one engine, two delivery modes, identical answers.

---

## Phase 9: Polish & Cross-Cutting Concerns

- [ ] T047 [P] Correct the falsified inline contracts in `crates/ipc/src/protocol.rs`: `:128-134` (counters no longer zero after restart) and `:87-89` (describe the ownership guard; the claimed PTY→`UnknownJob` routing never existed, and `CommandService` is not a real type)
- [ ] T048 [P] Verify the transport-failure remedy text at `crates/mcp/src/tools.rs:3700-3702` and its pinned test at `:7690` remain valid now that status is routed rather than rejected
- [ ] T049 Record the unbounded `live` map as a tracked follow-up — no `remove` exists, and `crates/daemon/src/ipc/handlers/runtime.rs:20` documents that bindings linger after exit. Out of scope to fix here; in scope to write down.
- [ ] T050 Attempt the Windows job-object regression test; if fault injection cannot be built without production code reaching test-only logic (Constitution VI), instead record the ownership invariant and register a tripwire in `scripts/windows-gate.ps1` per `CONTRIBUTING.md` §6.1
- [ ] T051 Assign a source-status label to every behavior touched. `unknown` is a hard fail at commit (Constitution VI).
- [ ] T052 Run `quickstart.md` scenarios 1-6 against a live daemon and capture evidence
- [ ] T053 Run `pwsh scripts/linux-gate.ps1` (linux gate via WSL)
- [ ] T054 Run `pwsh scripts/windows-gate.ps1`
- [ ] T055 Write the evidence report per `TESTING.md` §10: branch, files changed, PASS/FAIL per command, source-status notes, risk-register reference. A report without source-status notes is incomplete.
- [ ] T056 Open a pull request. Branch protection requires it — direct pushes to `main` bypass 8 required status checks.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (P1)**: no dependencies
- **Foundational (P2)**: blocks every story
- **US2 (P3)**: after Foundational. **Must precede US3.**
- **US1 (P4)**: after Foundational. Independent of US2.
- **US3 (P5)**: after US2 — otherwise `observed` certifies a cross-lane falsehood
- **US4 (P6)**: after Foundational (needs T005 + T003 index). Supplies US3's `lost` value.
- **US5 (P7)**: after US4 conceptually (complementary coverage) and after T004 for `end_cause`
- **US6 (P8)**: after US1 (reconstruction must exist before it moves)
- **Polish (P9)**: last

### Critical path

`T003 → T004 → T021/T022 → T024 → T046`

### Parallel Opportunities

- T005, T006, T008 are independent files within Foundational
- T010/T011, T018/T019/T020, T026/T027, T033/T034/T035, T038/T039/T040 are test files, all `[P]` within their story
- US1 and US2 can be developed concurrently once Foundational lands
- T031, T032, T047, T048 touch distinct files and parallelize in Polish

### Ordering constraints that are NOT negotiable

1. T022 before T024 — reconstruction must read a correct count, not bake in the off-by-one.
2. US2 before US3 — see above.
3. T042/T043 before T039 — the stale-replacement test needs the verb.
4. T053/T054 last — both gates are mandatory because this change touches `cfg(windows)` code **and** adds tests (`CONTRIBUTING.md` §6.1).

---

## Implementation Strategy

### MVP

Both P1 stories: **US2 then US1**. US2 removes a live falsehood; US1 removes the
reported 40-minute loss. Foundational plus those two phases is a coherent,
shippable increment.

### Incremental delivery

1. Setup + Foundational → vocabulary and schema exist
2. US2 → no lane lies about counters
3. US1 → reconstructed outcomes become usable (**MVP complete**)
4. US3 → provenance becomes explicit
5. US4 → lost becomes diagnosable
6. US5 → planned deaths recorded
7. US6 → delivery modes converge
8. Polish → contracts corrected, gates green, evidence recorded

### Notes

- No item here is optional. Scope reductions proposed during adversarial review
  (cutting the trust taxonomy, lost-detection, and the quiesce verb) were
  rejected by operator directive: a deferral is only acceptable when an item is
  genuinely not needed for the plan to succeed, not when it merely saves work.
- Two exclusions are recorded and are **not** deferrals: durable tail persistence
  is excluded on Constitution III grounds (decision D2), and proposal 2 is merged
  into US1 rather than dropped.
- Commit after each task or logical group. Stop at any checkpoint to validate.
