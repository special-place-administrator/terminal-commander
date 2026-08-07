# Adversarial review — TC-B3 remediation plan

Target: the "Fourth review — implementer verification" section of
`docs/audits/2026-08-06-orphaned-job-reports-exit-0.md`.

**Reviewer model:** Kimi (Moonshot), three independent lenses, read-only, each
verifying every `file:line` against source at `2ebd73e`.

**Deviation from the `adversarial-review` skill, stated for the record.** The
skill mandates Codex as the opposing model. Codex was quota-exhausted (CLI and
MCP share auth; resets 2026-08-10) and Gemini returned `IneligibleTierError`, so
Kimi was substituted — still genuinely cross-model, which is the constraint's
purpose. The skill also loads `brain/principles.md`, which does not exist in this
repository; `.specify/memory/constitution.md` v2.1.0 was substituted as the
governing principle source.

## Intent

Produce a correct, minimal, constitution-compliant remediation plan for the
TC-B3 evidence-stripping defect that a future implementer can execute without
re-deriving the analysis — including correcting claims in the source report that
would otherwise produce silently-broken fixes.

## Verdict: REJECT

The three factual corrections (W1, W2, W3) are confirmed unanimously and
independently by all three lenses. **Two of the three proposed remedies are
rejected with consensus**, and one endorsed proposal is self-defeating as
specified. The analysis holds; the prescriptions do not.

## Findings

### H1 — `Option<u64>` counters are backwards-incompatible, and unnecessary

Raised by all three lenses (Skeptic: high).

`frames_total`, `frames_stdout`, `frames_stderr`, `bytes_total` and
`events_emitted` are bare `u64` with no `#[serde(default)]`
(`crates/ipc/src/protocol.rs:111-115`); only later-added fields carry defaults
(`restarted`, `:135`). serde fails deserialization when a non-defaulted field is
absent, so `skip_serializing_if` makes an old client hard-error exactly when the
daemon omits a counter to say "I don't know".

The failing direction is a standing topology, not a hypothetical:
`replace_if_stale` replaces only *older* daemons (`crates/mcp/src/main.rs:142-148`),
so old-adapter/new-daemon persists. Result: every status poll for such a job
becomes a bare IPC error — strictly worse than a confident zero.

The fourth reviewer's claim that this was "serde-skipped so existing clients see
today's shape" is **wrong**: today's absence case is a hard `0`, not an absent key.

**Recommendation: drop the wire change entirely.**

### H2 — Rejecting non-`Process` ids from `command_status` destroys a capability

Raised by all three lenses (Skeptic: high).

There is no `pty_command_status` tool (`crates/mcp/src/tools.rs:284-299` exposes
only `start`/`write_stdin`/`stop`/`list`), and `handle_pty_command_list` filters
terminal jobs out (`crates/daemon/src/ipc/handlers/pty.rs:270-278`). So
`command_status` is today the **only** surface returning a finished PTY job's
`state`/`exit_code`/`duration_ms` — and per the fourth reviewer's own measurement
those three fields are *truthful*. Only the counters lie. The proposed remedy
destroys the truth along with the lie.

It also composes into a new falsehood: post-reject, a PTY id falls to the
`UnknownJob` fallback with no receipt and no `command_start` audit row, so
corrected proposal 6 classifies it "genuinely unknown id" — about a job the
daemon itself started.

Internal callers are safe (`ipc/handlers/runtime.rs:17` iterates
`command.live_jobs()`; `subscriptions/pull.rs:448-450` uses lane-specific
liveness; `ipc/server.rs:1045` is a combed job). The blast radius is entirely
external MCP callers, which the plan never named. One further external surface:
the transport-failure remedy at `crates/mcp/src/tools.rs:3700-3702`, pinned by a
test at `:7690`, tells agents to call `command_status` after *any* mutating op —
including PTY ops.

**Recommendation: route, don't reject.** Dispatch the status read by
`source_type` to the owning runtime. `PtyRuntime` already reads real
`frames_total`/`bytes_total` off the probe (`crates/daemon/src/pty_command.rs:756-771`),
so the PTY lane gets real data, no wire change, no capability loss.

### H3 — Proposal 7's `abandoned` state has nowhere to land

Raised by the Minimalist.

`restart_marked_status_from_receipt` maps any unrecognized `terminal_state` label
to `JobState::Failed` (`crates/daemon/src/ipc/handlers/command.rs:203-208`), and
`JobState` has no `Abandoned` variant (the match at `command.rs:1941-1949` is
exhaustive over five). So writing `terminal_state: "abandoned"` rows surfaces
gracefully-abandoned jobs as `state: failed` — **a new false-red of exactly the
class this document exists to eliminate**.

**Recommendation: proposal 7 is blocked until the representation is decided.**
Either add a `JobState` variant (which proposal 6's own shape guidance argues
against) or carry abandonment outside `JobState`.

### M1 — The working W2 fix is the conjunction neither document states

Raised by all three lenses.

The original report says "move the persist". The fourth reviewer says moving is a
no-op and to pass a different value. **Both halves are required and neither
document states them together.** The persist at `command.rs:1489` sits *outside*
the `is_ok()` guard at `:1508-1513`, so the fourth reviewer's parenthetical
("already inside an `is_ok()` guard") is wrong. An implementer computing
`rule_driven_events + 1` inline at `:1489` overcounts whenever `bucket_append`
fails.

**Recommendation:** move the persist below the append block and pass
`final_metrics.events_emitted`; keep `rule_driven_events` untouched for the
TCE-ERG-1 gate at `:1403`. The `:1457` early-return site keeps the reap-time
count, since `stop()` performs no append (`:1639`).

### M2 — Principle I is the wrong authority

Raised by the Architect. Principle I governs the *delivery* boundary; both
`CommandRuntime` and the IPC handlers live in the same crate, so handler-vs-runtime
placement is internal layering the principle is silent on. The citable
justification is `docs/EMBEDDING.md:49-50` (documenting `state.command.status` as
the embedder's status read) plus Additional Constraints
(`.specify/memory/constitution.md:193-196`). Conclusion right, citation wrong.

### M3 — The reframing dissolves once routing lands

Raised by all three lenses. In *both* instances the daemon actually knows: the
reconstruction gets real counters from V0008, and the PTY counters exist in the
lingering binding. The zero is a wiring artifact
(`.unwrap_or_default()`, `crates/daemon/src/command.rs:1858`), not absence of
knowledge. "Cannot express I don't know" is therefore over-generalized from two
instances, both of which the data-populating fixes remove.

### M4 — Moving reconstruction makes a read API perform a write

Raised by the Architect. `mark_job_receipt_restarted`
(`ipc/handlers/command.rs:223`) is a write on the status read path; relocating
reconstruction into `CommandRuntime::status` moves that write into the engine API
and changes the `UnknownJob` contract for all three internal callers. Worth one
explicit sentence in the plan.

### Low findings (accepted, folded into the plan)

1. **The TCE-ERG-1 regression IS test-covered.** The fourth reviewer's "no
   existing test on that path would flag it" is false —
   `crates/daemon/tests/command_status_lifecycle.rs:246-253` and
   `crates/mcp/tests/mcp_inline_rules_e2e.rs:425` both assert receipt non-null.
2. **The `reconstructed == live - 1` invariant is natural-exit only.** On the
   `stop()` path the persisted count can exceed the last live value. Test 1 must
   scope the assertion.
3. **Proposal 6's predicate needs `decision='allow'`** and is combed-lane only;
   PTY and watch audit as `pty_command_start` (`pty_command.rs:389`) and
   `file_watch_start` (`file_watch.rs:469`). The three-row decision table needs a
   lane qualifier.
4. **Lost-detection is best-effort** — audit emits are swallowed
   (`let _ = self.audit.emit`, `command.rs:787`). Failure direction is safe
   (JobLost degrades to UnknownJob, never the reverse); state it.
5. **Persisting the no-silence tail extends raw output into SQLite.**
   Constitution III sanctions the tail *on the wire*; its durable lifetime is
   undiscussed.
6. **"Live regression of the S1 fix" is overstated** — S1 was same-lane; nothing
   regressed. The comment was written as if the lane boundary did not exist.
7. **Doc-comment debt:** `protocol.rs:128-134` becomes false once counters
   persist, and `protocol.rs:87-89` names `CommandService`, a type that does not
   exist.
8. **The secret-leak concern was over-dramatized.** Only the routing premise is
   false; the conclusion holds, because receipts are built solely in the combed
   waiter and the live-map miss yields `receipt: None` for PTY ids.
9. **Alternative guard.** Requiring `live`-map presence (as `stop()` already does
   at `command.rs:1613-1614`) expresses ownership without depending on
   `SourceType`. Note `live` is never evicted — no `remove` exists — which is an
   unbounded-growth smell worth its own item.

## What went well

- All three factual corrections (W1, W2, W3) verified exactly by three
  independent lenses, including the dead-code finding, the by-value parameter
  trap, and the shared-`JobManager` cross-lane read.
- The Skeptic specifically probed W3 for concurrency weakness and found none: the
  `natural_completion_pending` mask (`command.rs:1886-1897`) requires a binding in
  CommandRuntime's `live` map, which PTY jobs never have, so the zeroed counters
  are deterministic rather than racy.
- `metrics_json TEXT` over explicit columns was endorsed without objection:
  `final_signal_counts` is the direct precedent and nothing queries on metrics.
- The sequencing (W3 before proposal 1) was confirmed dependency-correct.

## Lead judgment

| # | Finding | Ruling |
|---|---|---|
| H1 | `Option<u64>` compat break | **Accept** — drop the wire change; `outcome_trust` already carries expressiveness additively |
| H2 | Reject destroys PTY readback | **Accept** — remedy changes from *reject* to *route to owning runtime* |
| H3 | `abandoned` maps to Failed | **Accept** — proposal 7 blocked on a representation decision |
| M1 | W2 needs the conjunction | **Accept** — my parenthetical was wrong; same error class I caught in the source report |
| M2 | Wrong constitutional authority | **Accept** — cite EMBEDDING.md + Additional Constraints |
| M3 | Reframing over-generalizes | **Accept** — routing dissolves it; the root cause is wiring, not wire shape |
| M4 | Read API performs a write | **Accept** — record explicitly |
| L1-L9 | see above | **Accept** — all fold into the plan |

**Rejected: the Minimalist's scope cuts** (cut proposals 1, 2, 6 and
`QuiesceForReplace`; skip spec-kit). The reasoning is sound engineering, but it is
overridden by an explicit operator directive that deferrals must not be used to
reduce work, and that any item needed for the plan to succeed must be planned and
coded. Two specifics:

- Cutting proposal 1 rests on its third value (`lost`) being dead on arrival. That
  argument dissolves once proposal 6 is mandated — the third state becomes live,
  and a bool cannot express it.
- `QuiesceForReplace` is the only half of proposal 7 that covers the *actual*
  incident trigger (`replace_if_stale` hard-kill). The Minimalist correctly notes
  the graceful-drain half would not have fired in the incident — which is an
  argument for keeping `QuiesceForReplace`, not cutting it.

**Partially rejected:** proposal 2 is not "cut" but *merged* into proposal 3, which
the source document already specifies. No work is dropped.

## Net effect on the plan

Two remedies replaced, one proposal blocked pending a decision, and eight new work
items surfaced that were invisible to a `file:line` checklist:

1. Route status by `source_type` to the owning runtime (replaces the reject).
2. Decide the `abandoned` representation before proposal 7.
3. Update the MCP transport-failure remedy text and its pinning test.
4. Widen proposal 6's predicate per lane and add `decision='allow'`.
5. Resolve Constitution III implications of durable tail persistence.
6. Scope the off-by-one test assertion to natural exit.
7. Sweep the stale doc comments on `CommandStatusResponse` / `CommandReceipt`.
8. Record the unbounded `live` map as a separate item.
