# Phase 0 Research: Status Evidence and Trust

All findings below were verified against source at `2ebd73e`/`d98daa6`. Where a
prior document asserted something that turned out to be false, the correction is
recorded rather than silently fixed.

---

## R1 — The event-count fix requires a conjunction neither prior document stated

**Decision**: move the natural-exit persist below the lifecycle-append block and
pass the post-append `final_metrics.events_emitted`. Leave the `rule_driven_events`
variable untouched. The early-return persist keeps the reap-time count.

**Rationale**: three facts, each verified.

1. `persist_job_receipt` takes `rule_driven_events: u64` **by value**
   (`crates/daemon/src/command.rs:1939`) and never reads `final_metrics`. Both
   call sites pass the variable captured at `:1398`. So *relocating the call
   alone changes nothing* — the source audit's "move the persist" advice is
   insufficient on its own.
2. That same variable gates the no-silence receipt at `:1403`
   (`if rule_driven_events == 0`). Redefining it post-bump would make it `>= 1`
   for every command and silently disable the TCE-ERG-1 carve-out. So "just
   change the value" is insufficient on its own too.
3. The `+1` lives inside an `is_ok()` guard at `:1508-1513`. The persist at
   `:1489` sits **outside** that guard. Computing `rule_driven_events + 1` inline
   at the current site would overcount whenever `bucket_append` fails.

**Correction recorded**: the fourth-review parenthetical claiming the persist is
"already inside an `is_ok()` guard" is wrong. Both prior documents held exactly
half of the working fix and called the other half wrong or sufficient.

**Two sites, two counts**: `stop()` calls `jobs.cancel` and discards the draft
(`:1639`) with no `bucket_append`, so the early-return persist at `:1457` must
*not* add one. Only the natural-exit path does.

**Alternatives considered**: making the append infallible (larger blast radius,
changes bucket semantics); persisting both counts (two sources of truth for one
number).

---

## R2 — Lost-detection reads an audit row that exists, under a different name

**Decision**: predicate on `action IN (<per-lane start actions>) AND subject = <job id> AND decision = 'allow'`, backed by a new `(action, subject)` index and a subject-capable audit read.

**Rationale**: the source audit claimed `router.rs:211-214` audits `job_start` on
every job creation. **It does not.** `Router::job_start` is dead in production —
its only callers are `router.rs:331` (unit test) and
`crates/daemon/tests/audit_router.rs:132`. All three runtimes bypass the router
and call `JobManager::start` directly (`command.rs:1322`, `pty_command.rs:535`,
`file_watch.rs:452`).

The rows that *do* exist, per lane:

| Lane | Action | Subject |
|---|---|---|
| Combed | `command_start` (`command.rs:1346-1357`) | job id |
| PTY | `pty_command_start` (`pty_command.rs:389`) | job id |
| Watch | `file_watch_start` (`file_watch.rs:469`) | job id |

Implemented literally from the source audit, the lookup would match nothing
forever, and `JobLost` would never fire — while a test that hand-seeded a
`job_start` row would pass.

**Three supporting facts**:
- `V0003__audit.sql:18-23` indexes `timestamp`, `action`, `decision` — **not**
  `subject`. Without a new index the lookup degrades to a scan of every start row.
- Audit rows are **never pruned** (no `DELETE` against `audit_records`), so
  detection has no TTL — but the scan grows unboundedly, which makes the index
  load-bearing rather than cosmetic.
- `AuditReadRequest` (`crates/store/src/audit.rs:133-136`) exposes
  `action_filter` and `decision_filter` but **no subject filter**, so a store-side
  read capability must be added, not just an index.

**`decision = 'allow'` is defensive, and necessary**: `command_start` is also
written with decision `error` on the spawn-failure path (`command.rs:1255-1258`),
though with an argv-derived subject rather than a job id. Nothing in the schema
prevents a future job-id-subject row with another decision.

**Failure direction**: audit emits are swallowed (`let _ = self.audit.emit`,
`command.rs:787`), so detection is best-effort. An inconclusive lookup must
degrade to `unknown`, never to a terminal outcome.

---

## R3 — Route the status read to the owning lane; do not reject

**Decision**: `CommandRuntime::status` answers only for jobs it owns; a request
for another lane's job is dispatched to that lane's runtime, which returns real
counters. No caller loses a capability.

**Rationale**: one `Arc<JobManager>` is minted at `state.rs:201` and shared into
CommandRuntime (`:279`), WatchRuntime (`:300`) and PtyRuntime (`:311`).
`JobManager::get` (`crates/core/src/job.rs:288`) has no `source_type` filter, so
CommandRuntime finds other lanes' jobs; only its private `live` map misses, and
that miss becomes `.unwrap_or_default()` at `command.rs:1858` — zeros, reported
alongside `restarted: false`.

**Why the obvious remedy was rejected.** Answering non-owned ids with a bare
`UnknownJob` was the initial plan. Three independent reviews and the constitution
converge against it:

- There is **no** `pty_command_status` tool, and `handle_pty_command_list`
  filters terminal jobs out (`ipc/handlers/pty.rs:270-278`). So `command_status`
  is today the *only* surface returning a finished PTY job's state, exit code and
  duration — and those three fields are truthful. Rejecting destroys the truth
  along with the lie.
- It composes into a *new* falsehood: post-reject, a PTY id reaches the lost
  lookup with no receipt and (under a combed-only predicate) no matching start
  row, and is classified "genuinely unknown" — about a job the daemon started.
- Constitution VII forbids it directly: *"never a bare error that discards a live
  job."*

**The data already exists**: `PtyRuntime::list` reads real `frames_total` /
`bytes_total` off the probe (`pty_command.rs:756-771`). The zero is a wiring
artifact, not absence of knowledge.

**Internal callers verified safe** (none passes a cross-lane id):
`ipc/server.rs:1045` (self-check, a combed job it just started),
`ipc/handlers/runtime.rs:17-18` (iterates `command.live_jobs()`, i.e. its own
map), and `subscriptions/pull.rs:428-429` (guarded by `ProbeKind::Command`).

**Ownership guard shape**: two are defensible — filter on `source_type`, or
require presence in the runtime's own `live` map (which `stop()` already does at
`command.rs:1613-1614`). The `live`-map form expresses "I only report metrics for
jobs I own" without depending on the lifecycle enum. It is safe **today** only
because combed bindings are never evicted — no `remove` on that map exists, which
`ipc/handlers/runtime.rs:20` independently documents as "bindings linger after
exit". That is an unbounded-growth smell tracked as its own task. The plan states
the invariant, not just the mechanism.

---

## R4 — Provenance ships as an additive closed enum; optional counters are rejected

**Decision**: add `outcome_trust`, a closed enum with `observed`,
`reconstructed`, `lost`, `abandoned`, defaulted for compatibility. Retain
`restarted` as a serde-defaulted compat alias with its original meaning. Do
**not** make counters optional.

**Rationale**: the counters (`crates/ipc/src/protocol.rs:111-115`) are bare `u64`
with **no** `#[serde(default)]` — only later-added fields carry defaults
(`restarted`, `:135`). Making them `Option<u64>` with skip-on-none removes the key
from the wire, and serde then fails to deserialize the whole response on any
client compiled against the current schema — precisely in the case the change
exists to serve.

That failing direction is a standing configuration, not a hypothetical:
stale-replacement replaces only *older* daemons
(`crates/mcp/src/main.rs:142-148`), so old-adapter/new-daemon persists. The
result would be a bare IPC error on every affected status poll — strictly worse
than a confident zero.

**Correction recorded**: the fourth review's claim that this would be
"serde-skipped so existing clients see today's shape" is wrong. Today's absence
case is a hard `0`, not an absent key.

**The need also disappears.** Once R3 routes the read and evidence is persisted,
no served path lacks metrics except pre-migration legacy rows, which are handled
by R7. Optionality would be a compat-breaking wire change serving a shrinking
legacy cohort.

**Precedent for shape**: `run_and_watch` already carries `degraded` /
`recover_hint` on **every** payload, normal and degraded alike, pinned by
`run_and_watch_normal_terminal_is_complete_and_a_strict_superset`
(`crates/mcp/src/tools.rs:6345`). `outcome_trust` follows the same rule — present
on every response, never conditionally omitted. Note these are MCP-layer fields;
`outcome_trust` is an IPC-wire field, so the two are complementary, not
duplicative.

**Constitution VII** additionally requires "closed typed codes plus bounded
numeric or safe-enum fields", which rules out a free-text provenance note.

---

## R5 — `QuiesceForReplace` mirrors `Shutdown`'s existing posture

**Decision**: add the verb with exactly the authorization posture `Shutdown`
already has — no `[policy.caps]` flag, protected by the local-only endpoint and
attested peer identity.

**Rationale**: `IpcRequest::Shutdown` is dispatched with no policy gate
(`crates/daemon/src/ipc/server.rs:854-857`: `state.trigger_shutdown()` directly).
Its protection is Principle IV's boundary, not Principle II's capability flags.
`QuiesceForReplace` is strictly weaker — it spawns nothing, kills nothing, and
only asks the daemon to write records about its own in-flight jobs. Introducing a
capability flag for it would be a new gate with no new capability behind it.

**Why a handshake at all**: the replacer cannot know the outgoing daemon's
in-flight set — it lives in that daemon's memory, not on disk. So the outgoing
daemon must write its own records. `replace_if_stale` already handshakes before
`hard_kill` (`crates/supervisor/src/replace.rs:740`), so the verb slots into an
existing exchange. Timeout falls back to current behaviour and never blocks the
replacement.

---

## R6 — Windows job-object regression guard may need a documented invariant instead

**Decision**: attempt a fault-injected test; if injection cannot be built without
production code reaching test-only logic, record the ownership argument as a
documented invariant plus a `scripts/windows-gate.ps1` tripwire.

**Rationale**: Principle VI (NON-NEGOTIABLE) forbids production paths reaching
test-only logic, and `CONTRIBUTING.md` §6.1 explicitly sanctions the
tripwire-plus-invariant fallback for "Windows cfg sentinels that headless CI
cannot exercise live". The property being guarded currently holds by ownership:
`ProcessProbe` holds `_job: Option<Arc<JobHandle>>` (`crates/probes/src/process.rs:212`),
`KILL_ON_JOB_CLOSE` fires only when the last `Arc` drops (`:181-190`), and
`drive_to_exit` takes the probe by value and holds it across `probe.wait()`
(`command.rs:1910`). So the waiter owns a live handle for the whole wait and the
job cannot close underneath it.

This is a regression guard for a property that holds, not a fix for a live defect.

---

## R7 — Pre-migration receipt rows are reported honestly, never backfilled

**Decision**: rows written before the migration carry no evidence. They report
`outcome_trust: reconstructed` with the evidence fields absent-by-value, and the
agent-facing contract states that reconstructed outcomes from older rows may lack
counters.

**Rationale**: the evidence was never captured; inventing it would be exactly the
dishonesty this feature removes. The cohort is bounded and shrinking — every new
terminal transition writes full evidence.

---

## R8 — Corrections carried forward from adversarial review

Recorded so the implementer does not re-derive them:

- **The TCE-ERG-1 regression IS test-covered.** The fourth review's claim that no
  existing test would flag it is false —
  `crates/daemon/tests/command_status_lifecycle.rs:246-253` and
  `crates/mcp/tests/mcp_inline_rules_e2e.rs:425` both assert the receipt is
  non-null. An implementer who broke the gate would go red immediately.
- **The `reconstructed == live - 1` invariant is natural-exit only.** On the
  `stop()` path, `stop()` publishes a live snapshot at `command.rs:1635` before
  cancel while the persist uses the reap-time count, so the persisted value can
  exceed the last live value. Test assertions must scope accordingly.
- **Constitutional authority for the engine-side move** is `docs/EMBEDDING.md:49-50`
  plus Additional Constraints (`constitution.md:193-196`), not Principle I's
  delivery-boundary text. Conclusion unchanged; citation corrected.
- **Moving reconstruction into the engine makes a read API perform a write** —
  `mark_job_receipt_restarted` (`ipc/handlers/command.rs:223`). This must be
  explicit, and the `UnknownJob` contract changes for all three internal callers.
- **The secret-leak concern was over-stated.** Only the routing premise in
  `crates/ipc/src/protocol.rs:87-89` is false; the conclusion holds, because
  receipts are built solely in the combed waiter and the live-map miss yields
  `receipt: None`. That comment also names `CommandService`, a type that does not
  exist — further evidence it is stale.
- **`command_output_tail` answers cross-lane and returns *correct* data.** It must
  be left working; only `command_status` lies. A blanket lane guard would be wrong.
- **The MCP transport-failure remedy** (`crates/mcp/src/tools.rs:3700-3702`,
  pinned at `:7690`) tells agents to confirm state via `command_status` after any
  mutating op, including PTY ops. Routing keeps that advice valid; rejecting would
  have broken it.
