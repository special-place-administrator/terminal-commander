# Restart reconstruction returns a true outcome with its evidence stripped

**Reported by:** a Claude Code session using TC as its build/test harness on the
`symforge` repo, 2026-08-06.
**TC version:** `0.1.86` (from `health`).
**Platform:** Windows 11 Pro 10.0.26200.

> **This report was substantially wrong on first filing and has been rewritten
> against the source.** The original headline — "a lost run reads as a
> successful one" — is **retracted**; the code does not do that. The retraction
> and what actually happened are kept below, because the reporter's wrong
> inference is itself the usability finding.

**Severity (revised):** medium. Not a false-green. It is a **false-red /
evidence-stripping** defect that cost the reporting session ~40 minutes of
unnecessary re-runs, plus one narrow theoretical false-green worth a targeted
test (see "The one real false-green risk").

> **Revision note.** This document has been revised three times, each time
> against source, and three of the reporter's own claims have been retracted:
> the false-green headline, the "zero bytes proves it never ran" heuristic, and
> the claim that the MCP *tool description* misleads agents (it is a code comment
> agents never see — see proposal 5). Where a claim below is not backed by a
> `file:line`, treat it as unverified.
>
> **Fourth review (implementer verification).** Every `file:line` above was
> re-checked against source before any code was written. The false-green
> retraction holds and the framing is broadly right, but **two load-bearing
> implementation claims are wrong** and the **PTY scope caveat is false** — the
> same evidence-stripping defect is already live on the PTY path with no restart
> involved. See "Fourth review" at the end.

**Scope caveat — receipts are combed-lane only.** Both `persist_job_receipt`
call sites are in `crates/daemon/src/command.rs`. PTY commands, shell sessions
and file watches persist no receipt, and `handle_command_status` deliberately
routes PTY job ids to `UnknownJob` (`crates/ipc/src/protocol.rs:87-89`). Every
proposal below is scoped to the combed process lane unless the receipt mechanism
is extended to the PTY waiter, which mirrors the same finalization structure.

---

## Consumer scope

**Every finding below is scoped to the MCP/IPC surface.** TC's second delivery
mode — an in-process host embedding the `terminal-commanderd` library
(`.specify/memory/constitution.md:54-58`, `docs/EMBEDDING.md`,
`examples/embed_in_process.rs`) — does not reach the TC-B3 read path at all:
`restart_marked_status_from_receipt` is a private `fn`
(`crates/daemon/src/ipc/handlers/command.rs:194`), its only caller is
`pub(in crate::ipc::server)`, and `mod handlers;` is private.

That narrows nothing in practice — MCP-driven agent harnesses are the entire
consumer population today, and the reporter was one of them. It is stated only so
the audit is not read as covering TC's whole surface.

**Note on the trigger.** The precondition is not literally "daemon restarted" —
it is `UnknownJob`: the job is absent from the in-memory `JobManager` while a
receipt survives on disk. `DaemonState::bootstrap` mints a fresh
`JobManager::new()` over a persistent store, so **any** new `DaemonState` over
the same `data_dir` reproduces it, restart or not.

---

## Related defect: the write half is in the engine, the read half is not

Found while scoping the above; reported here because it is the same subsystem.

`persist_job_receipt` (`crates/daemon/src/command.rs:1933`, called at `:1457`
and `:1489`) runs inside `CommandRuntime` — the engine. An in-process embedder
therefore **accumulates `job_receipts` rows** as a side effect of running
commands.

But `CommandRuntime::status` (`command.rs:1841`) has no counterpart. It reads the
in-memory `JobManager`, returns a bare `UnknownJob` when the job is gone, and
hardcodes:

```1876:1878:crates/daemon/src/command.rs
            // TC-B3: the live in-memory path is never a restart-reconstructed
            // result; the persisted-receipt fallback sets this in the handler.
            restarted: false,
```

The comment is accurate and is precisely the problem: the fallback lives in the
IPC handler, which an embedder does not go through. So an embedding host writes
durable evidence it has no supported typed API to read back — its only route is
`state.store.get_job_receipt` plus reimplementing the terminal-state string
mapping by hand.

Not urgent (no embedder exists today), but worth fixing alongside the main issue,
since any fix to the reconstruction shape should land in `CommandRuntime` rather
than only in the handler — otherwise the two delivery modes drift further apart.

---

## What actually happens

A `command_status` for a job the daemon no longer holds in memory goes through
TC-B3 `restart_marked_status_from_receipt`
(`crates/daemon/src/ipc/handlers/command.rs:194-247`). That function returns a
**truthful terminal outcome read from SQLite**, but hardcodes every corroborating
field to empty:

```
frames_total: 0, frames_stdout: 0, frames_stderr: 0, bytes_total: 0
duration_ms: None
receipt: None
probe_id: ProbeId::default()     // fresh id, hence a ULID later than the job's
exit_code: row.exit_code         // REAL — from the persisted receipt
events_emitted: <from row.final_signal_counts>
restarted: true
```

So a genuine 20-minute passing suite and a hypothetical lost run would present
with the same zeros. The reporter saw the zeros, concluded "a 20-minute suite
cannot produce zero bytes, therefore it never ran", discarded a **passing**
result, and re-ran it. Twice.

---

## Why it is NOT a false-green (the retraction)

Verified against source:

- `drive_to_exit` (`crates/daemon/src/command.rs:1910-1923`) only yields
  `ProbeOutcome::Exited { code: status.code(), .. }` **after `probe.wait()`
  actually reaps the child**. There is no path that fabricates `0`.
- Both `persist_job_receipt` call sites (`command.rs:1457`, `command.rs:1489`)
  sit *after* that reap, on a real terminal transition.
- The migration and module docs agree: receipts are written
  *"on every terminal transition (Exited / Cancelled / Failed)"*
  (`crates/store/migrations/V0007__job_receipt.sql:7-8`,
  `crates/store/src/job_receipt.rs:10`).
- `restart_marked_status_from_receipt` `?`-returns `None` when no receipt row
  exists (`command.rs:201`), so a job killed in flight — daemon death takes the
  waiter with it, no receipt — **cannot** surface as a terminal success.

**Therefore both reported incidents were genuine passes.** Non-zero
`events_emitted` (115 and 114, against 116 for a healthy complete run) is itself
proof a receipt existed, which proves a terminal transition occurred.

The reporter's original "zero bytes ⇒ never ran" heuristic is retracted: those
zeros are hardcoded on the reconstruction path regardless of outcome.

---

## The incidents

| | Incident 1 | Incident 2 | Healthy run, same command |
|---|---|---|---|
| `job_id` | `job_019fd63c652e7e02b6eb1b2dd26d423e` | `job_019fd6f527b77cc0a50acecbc7197836` | `job_019fd72fe21578a1a7e74032f6537c72` |
| `exit_code` | 0 | 0 | 0 |
| `restarted` | true | true | false |
| `frames_total` | 0 | 0 | 4586 |
| `bytes_total` | 0 | 0 | 334421 |
| `duration_ms` | null | null | 641248 |
| `events_emitted` | 115 | 114 | 116 |
| `probe_id` prefix | `019fd65b` (> job) | `019fd72f` (> job) | `019fd72f` (== job) |

Command in all cases:
`cargo test --all-targets --no-fail-fast -- --test-threads=1`,
`cwd = E:\project\symforge-policy`.

Trigger, measured: at 15:12:28 `health` reported `uptime_secs: 293`, so the
daemon started ~15:07:35 — long after incident 2's job was launched. The operator
had restarted the MCP servers at ~15:06:25. `Get-Process cargo,rustc` was empty.

---

## The suspected false-green path — investigated and CLOSED

A reviewer raised this, and it was the last open route to a genuinely false
success, so it was traced rather than left as a caveat.

**The concern:** children run inside a Job Object with
`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`. If any shutdown ordering let the job handle
close while a waiter was still alive to observe `probe.wait()`, that waiter could
see a plausible `Ok(status)` for a child that was actually killed, and persist a
receipt claiming `exited`.

**Why it cannot happen:** ownership forbids it.

- `ProcessProbe` holds `_job: Option<Arc<JobHandle>>`
  (`crates/probes/src/process.rs:212`).
- `KILL_ON_JOB_CLOSE` fires only when the **last** `Arc` drops, via
  `JobHandle::Drop → CloseHandle` (`process.rs:181-190`).
- `drive_to_exit(mut probe: ProcessProbe)` (`crates/daemon/src/command.rs:1910`)
  takes the probe **by value** and holds it across `probe.wait().await`.

So for the entire duration of the wait, the waiter itself owns a live `Arc` to
the job handle. The handle cannot close underneath the very task that would
observe and persist the result.

The complementary branch is equally honest: if daemon shutdown aborts the
lifecycle task, the future is dropped, the probe drops, the child tree is torn
down — and the aborted waiter persists nothing. No receipt, no green.

Explicit cancel is separately safe: it uses `TerminateJobObject(.., 1)` and
lands as `ProbeOutcome::Cancelled`, which maps to `exit_code: None`
(`command.rs:1399-1402`).

**Conclusion: no path was found by which TC persists a fabricated success.** The
test in the list below is still worth adding as a regression guard, but it
guards a property that currently holds rather than fixing a live defect.

---

## Requested behaviour

Ordered by value-per-effort.

### 1. Make "reconstructed" impossible to mistake for "observed"

`restarted: true` is the only signal today, and its name reads as a benign
internal detail rather than "the evidence for this outcome is gone". A
first-class field is clearer:

```
outcome_trust: "observed" | "restart_reconstructed" | "lost"
```

Keep `restarted` as a compat alias. Agents should branch on one obvious field,
not infer from a conspiracy of zeroed counters.

### 2. Explain the reconstruction — but NOT by overloading `receipt`

**Corrected.** An earlier draft proposed stuffing a free-text note into
`receipt`. That would corrupt a typed contract: `CommandReceipt`
(`crates/ipc/src/protocol.rs:91-102`) is `{exit_code, lines_suppressed, tail,
tail_incomplete}` — the bounded no-silence *tail* for zero-rule commands, not a
message channel.

Two clean options:

- add a dedicated `reconstruction_note: Option<String>`, or
- **better**, fold this into proposal 3: persist the real `CommandReceipt` tail
  when one exists and return *that*, so the field keeps its meaning.

Worth noting for expectations: in both reported incidents `receipt` was `null`
**even live** — 115 rule-driven events means the zero-rule carve-out never
applied. So this proposal alone would have helped only as a label, which is
precisely why proposal 3 is the one that pays.

### 3. Persist the counters, so a reconstructed pass is *usable* — not just labelled

This is the item that actually recovers the lost 40 minutes, and it is easy to
mis-file as polish.

Labelling (1 and 2) stops an agent from over-trusting. It does **not** stop a
careful agent from re-running, because a labelled outcome with zero evidence is
still unverifiable — and any harness gating real work will re-run rather than
bank an unverifiable pass. Persisting `frames_total`, `bytes_total`,
`duration_ms` and the original `probe_id` on the receipt makes the difference
between "labelled reconstruction" and "usable reconstruction". With
`bytes_total: 334421` and `duration_ms: 641248` visible, this session would have
accepted the pass immediately.

Labels prevent false-green. Counters prevent false-red. They are different bugs.

**Everything needed is already in scope at the persist site:** `final_metrics`
(frames/bytes/suppressed), `duration_ms` from the job record's `exit_info`, the
`probe_id` from the binding, and the tail when one exists. Suggested shape: a
V0008 migration adding a `metrics_json TEXT` column — one `ALTER TABLE`, matching
the existing `final_signal_counts` JSON precedent. Explicit columns buy
queryability that nothing here needs.

**Implementation trap — move the persist after the lifecycle append.**
`persist_job_receipt` runs at `command.rs:1489`; the lifecycle append that does
`final_metrics.events_emitted.saturating_add(1)` runs *after*, at
`command.rs:1506-1513`. So a reconstructed status today reads **exactly one less**
than the same run observed live — which is the whole explanation for the
115/114-vs-116 numbers in the incident table, not run-to-run noise. If counters
are persisted at the current site without reordering, every reconstructed metric
inherits that off-by-one permanently.

### 4. Keep a real `exit_code: 0` after restart — do not force `null`

Some reviewers will propose forcing `exit_code: null` on every reconstruction for
maximum conservatism. **Recommend against.** This session is the counter-example:
that rule would have mandated re-running two suites that had genuinely passed,
which is exactly the 40 minutes already lost. Only *unobserved* outcomes deserve
`null`, and those already return `None` from this path rather than a green.

### 5. Write an agent-facing contract — cheapest fix, largest blast radius

**Corrected target.** An earlier draft of this report quoted
`crates/mcp/src/tools.rs:4914-4918` ("An honest terminal result, never an
error") and claimed the tool description misleads agents. That text is a Rust
`//` comment inside the payload builder. **Agents never see it.**

The actual description agents receive is the `#[tool(description = ...)]` string
at `crates/mcp/src/tools.rs:1525`, and it **never mentions `restarted` at all**:

> "Lookup bounded counters and exit info for a previously started job. Never
> returns raw stream text, with one exception: when the command finished and
> ZERO rules matched, a bounded exit receipt … is included so a no-rule command
> is never silent."

So agents receive a bare `"restarted": true` with **zero contract**. The fix is
not softening a misleading description — it is writing one where none exists,
which is strictly easier and strictly more valuable.

Suggested substance for `tools.rs:1525`:

> `restarted: true` means the daemon restarted after this job finished.
> `state`/`exit_code` are truthful, read from the persisted receipt; live
> counters and the receipt are not retained (zero/null). Treat the outcome as
> **true but unevidenced**: fine for information, re-run if it gates expensive
> work and you need the evidence.

That gives `restarted` the same mental model the surface already teaches for
`degraded` and `wait_exhausted`. Also update `run_and_watch`'s description if it
can surface a reconstructed status, and the user docs under `docs/mcp/`.

### 6. Make `lost` distinguishable from `UnknownJob` — read-side only

An earlier draft implied this needed the write path in proposal 7. **It does
not.** `router.rs:211-214` already durably audits `job_start` with the `job_id`
as subject on every job creation, into the persistent action-indexed audit table
(V0003). Post-restart, all three cases are separable from data that already
exists:

| On disk | Meaning |
|---|---|
| receipt row present | restart-reconstructed terminal (today's path) |
| no receipt, but a `job_start` audit row | **provably lost** — started, never reached a recorded terminal transition |
| neither | genuinely unknown id |

One `SELECT 1 … WHERE action='job_start' AND subject=?` on the `UnknownJob`
fallback closes it.

**Shape:** prefer a distinct IPC error code (`JobLost`) with structured detail
over a new `JobState::Lost`. `JobState` is a core enum threaded through the job
manager, and "the daemon lost the thread" is not a job lifecycle state. An error
code also fails closed for older clients.

### 7. Record abandonment at kill time — two sub-paths, different costs

**Corrected.** An earlier draft claimed a dying daemon "cannot write anything on
its way out". That is wrong for the graceful path: shutdown drains lifecycle
waiters (10 s `LIFECYCLE_DRAIN_CEILING`, `command.rs:267`) *before*
`shutdown_store` (`runtime.rs:376-377`). Inside that window the daemon knows its
non-terminal set and the store is still writable — it can write its own
`terminal_state: "abandoned"`, `exit_code: NULL` rows. **Trivial.**

The caveat applies only to the external hard-kill path: `replace_if_stale` calls
`hard_kill(pid)` (`crates/supervisor/src/replace.rs:744`).

And the earlier suggestion that *"the replacing daemon writes rows on the old
daemon's behalf"* has a flaw it did not note: **the replacer cannot know the
in-flight set** — it lives in the old daemon's memory, not on disk. The workable
shape is a pre-kill IPC verb (`QuiesceForReplace`): the replacer already
handshakes with the old daemon, so it asks the old daemon to write its own
abandoned rows, waits a bounded beat, then hard-kills as today. Timeout falls
back to current behaviour.

OS crash / external `TerminateProcess` stays unrecordable by construction — which
is fine, because proposal 6 detects exactly that case from data that already
exists. **6 and 7 are complements: 7 covers planned deaths, 6 covers unplanned
ones.**

---

## Test gap — narrower and different than first stated

**Correction to an earlier draft of this report.** The proposed repro was
"mid-run daemon death → status must not be green". That test is worth having, but
it would **not** have caught either incident here — those took the
completed-then-restarted path, which already refuses to go green.

Existing coverage (`crates/mcp/tests/ledger_compact_wait_restart.rs:260`,
`command_status_after_restart_returns_restart_marked_terminal`) runs `printf` to
completion, drops the daemon, and asserts a restart-marked **terminal** result.
It proves the reconstruction exists. It asserts nothing about whether that
reconstruction is *usable*.

Five tests, in order of value:

1. **Completed → restart → status carries usable evidence.** After proposal 3,
   assert reconstructed counters / `duration_ms` / `probe_id` match the
   pre-restart live status, and that `events_emitted` matches **including** the
   lifecycle event — which pins the off-by-one shut. This is the test that maps
   to the real incidents; the existing test's silence on counters is the gap that
   cost 40 minutes.
2. **Mid-run hard-kill → never green.** Start a sleeper, kill the daemon
   *process* (not graceful shutdown), restart, and assert status is
   `JobLost`/`UnknownJob` — never `{exited, 0}`. This would not have caught these
   incidents; it pins the retracted fear as a permanent invariant.
3. **Lost detection** (from proposal 6). `job_start` audit row present + no
   receipt ⇒ `JobLost`; neither ⇒ `UnknownJob`.
4. **Graceful abandonment** (from proposal 7). Sleeper + `Shutdown` IPC past the
   drain ceiling ⇒ post-restart status is `abandoned`/null, not `UnknownJob`.
5. **Windows job-object close.** A child killed by `KILL_ON_JOB_CLOSE` must never
   persist `terminal_state: "exited", exit_code: 0`. Needs a test-only
   fault-injection hook; if injection proves impractical, record the ownership
   argument above as a documented invariant with a review tripwire on
   `JobHandle` instead.

---

## Suggested sequencing

1. **Proposal 5** (docs) — hours of work, immediate blast radius across every
   agent.
2. **Proposals 3 + 2** — the substantive fix: V0008 migration, persist-site
   reorder, reconstruction populated from disk.
3. **Proposals 1 + 6** — trust taxonomy and lost detection, one wire change.
4. **Proposal 7** — abandonment; graceful path first, `QuiesceForReplace` second.

Tests land with each step. Nothing here should be skipped on the grounds that
labelling suffices — this incident is the proof that a labelled-but-empty outcome
still gets re-run.

---

## One design observation for whoever implements

The root choice was **"receipt = minimal backstop"** — V0007's own comment calls
it *"a compact job receipt … enough for a status poll AFTER a daemon restart"*.
Minimal turned out to be indistinguishable from empty, which is the entire
defect.

The target worth aiming at is **"receipt = complete terminal snapshot"**:
everything `command_status` can say about a finished job should be
reconstructible from disk, with `restarted` / `outcome_trust` demoted to
provenance metadata rather than a trust decision the agent has to make for
itself.

---

## Note on the response-shape proposals (detail for proposal 1)

Two shapes were suggested by independent reviewers; either satisfies the
requirement, which is that **one obvious field** carries the trust signal rather
than agents inferring it from zeroed counters.

- `outcome_trust: "observed" | "restart_reconstructed" | "lost"` — more
  expressive, folds the lost case into the same field. **Preferred**: the domain
  genuinely has three states, and a bool cannot express the third.
  Ship it with the first two values; add `"lost"` together with proposal 6, since
  until that detection exists the value would never be emitted honestly.
- `exit_code` plus `exit_code_verifiable: bool` — less expressive, but yields a
  directly gateable predicate: `exit_code_verifiable && exit_code == 0`.

Whichever is chosen, keep `restarted` as a serde-defaulted backward-compat alias;
the wire struct already documents it.

---

## Operational note for TC users (corrected)

`restarted == true` means **the outcome is true but its evidence was discarded**.
It does not mean the command failed, and it does not mean the command never ran.

Do **not** infer from `frames_total == 0` that nothing executed — that inference
is what this report retracts. If the result gates something expensive, either
re-run or wait for the counters to be persisted.

The most useful diagnostic available today is comparing `health.uptime_secs`
against the job's age: a daemon younger than the job proves a restart intervened.
That is how the trigger was established here, and it costs one call.

`events_emitted` on a reconstructed status is deterministically **one less** than
the same run observed live (the persist at `command.rs:1489` precedes the
lifecycle append at `:1506-1513`). That makes
`reconstructed == live_expected - 1` a checkable invariant rather than the "weak
corroboration" an earlier draft called it — though it is an implementation
artifact today, not a contract, and proposal 3 should remove it rather than
enshrine it.

**Nothing else survives.** There is no latent event history to fall back on:
`Router::bucket_append` writes only to the in-memory `BucketManager`
(`router.rs:105-120`), and `EventStore::append` (`crates/store/src/lib.rs:311`)
has no production caller — the events/buckets/FTS tables are dormant in the
daemon hot path. The receipt row and the audit log are the **only** durable
per-job records, which is what raises the stakes on proposal 3.

**Corrected trigger guidance.** "Restarting MCP servers kills in-flight TC jobs"
is too broad. Daemons are spawned detached and survive adapter exit. The kill
comes from `replace_if_stale` on adapter start
(`crates/mcp/src/main.rs:142-146`), which hard-kills only a **version-stale**
daemon.

That fits the measured timeline exactly: TC was updated, then the MCP adapter was
restarted, and the cold-starting adapter replaced the now-stale daemon underneath
the reporter's jobs (MCP restart 15:06:25 → daemon birth ~15:07:35).

So the dangerous sequence is specifically **update TC, then restart the MCP
adapter, while work is in flight**. An up-to-date daemon survives an MCP restart
untouched.

---

## Fourth review — implementer verification (2026-08-06)

Checked against source at `0.1.86` before writing any code, plus one live probe
against a running `0.1.86` daemon. Summary: **the retraction is correct and the
defect is real**, but two implementation claims would produce silently-broken
fixes if followed literally, and the scope caveat is false.

### Confirmed as written

`restart_marked_status_from_receipt` shape (`ipc/handlers/command.rs:194-247`);
both persist sites (`command.rs:1457`, `:1489`); `persist_job_receipt`
(`:1933`); `drive_to_exit` never fabricating an exit code (`:1910-1923`);
`?`-return on a missing receipt (`:201`); V0007 carrying no metrics columns;
`CommandReceipt`'s typed shape (`ipc/protocol.rs:91-102`), so proposal 2's
"don't overload `receipt`" correction is right; `tools.rs:1525` never mentioning
`restarted` and `tools.rs:4914-4918` being a `//` comment agents never see;
`CommandRuntime::status` hardcoding `restarted: false` with no engine-side
fallback (`:1841`, `:1876-1878`), so the "Related defect" is real; the Windows
job-object ownership argument (`probes/src/process.rs:181-190`, `:212`, probe
held by value across `.wait()`); graceful shutdown draining before
`shutdown_store` on **both** arms (`runtime.rs:376-377` unix, `:452-453`
windows), so proposal 7's graceful path is as cheap as claimed on the reporter's
platform; `EventStore::append` having no production caller (no append variant in
`StoreOp`; `Router::bucket_append` hits the in-memory manager at
`router.rs:115`); and the existing test asserting only `restarted` and `state`
(`mcp/tests/ledger_compact_wait_restart.rs:311-320`).

The **off-by-one is real**: `rule_driven_events` is captured at `command.rs:1398`,
the bump lands at `:1511`, the live binding is updated at `:1521`. Reconstructed
is deterministically live-minus-one. Audit rows are additionally **never pruned**
(no `DELETE` in `store/src/audit.rs`), so proposal 6's detection has no TTL.

### Wrong 1 — proposal 6 rests on dead code

The claim is that `router.rs:211-214` "already durably audits `job_start` … on
every job creation". `Router::job_start` is **never called in production**. Its
only callers are `router.rs:331` (unit test) and `daemon/tests/audit_router.rs:132`.
All three runtimes bypass the router and call `JobManager::start` directly:
`command.rs:1322`, `pty_command.rs:535`, `file_watch.rs:452`. **No `job_start`
row is ever written.**

The conclusion survives by a different route: `command.rs:1346-1357` emits
`audit("command_start", <job_id wire string>, "allow", …)` — durable, job-id
subject, and emitted inside `CommandRuntime`, so it covers the embed path too.
The predicate must therefore be `action='command_start'`, not `'job_start'`.

Why this matters beyond a citation swap: implemented literally the SELECT matches
nothing, `JobLost` never fires, and a test that seeds a `job_start` row by hand
passes while production stays broken permanently.

Additionally, V0003 indexes `timestamp`, `action`, `decision` — **not `subject`**.
Combined with unbounded audit growth, the lookup degrades to a scan of every
`command_start` row. Add a `(action, subject)` index in the same migration as
proposal 3's, so proposal 6 needs no migration of its own.

### Wrong 2 — the proposal 3 "implementation trap" is inverted

"Move the persist after the lifecycle append" is a **no-op**.
`persist_job_receipt` does not read `final_metrics`; it takes
`rule_driven_events: u64` by value (`command.rs:1939`) and the caller passes the
variable captured at `:1398`. Relocating the call changes nothing.

The dangerous move is the one the advice implies. `rule_driven_events` has a
second, correctness-critical consumer: `if rule_driven_events == 0` at `:1403`
gates the TCE-ERG-1 no-silence receipt. Redefine it post-bump and it is `>= 1`
for every command, so the no-silence receipt is **never emitted again** — a
silent regression of the one sanctioned exception to "TC never returns raw
output", failing as an absence that no existing test on that path would flag.

Correct shape: leave `rule_driven_events` alone and pass a separate persisted
count. The two sites need **different** values — the early return at `:1457`
(`stop()` already finalized) never reaches the lifecycle append, because `stop()`
calls `jobs.cancel` and discards the draft (`:1639`) without a `bucket_append`.
So the `+1` applies only at `:1489`, and only when the append actually succeeded
(it is already inside an `is_ok()` guard).

### Wrong 3 — the PTY scope caveat is false, and the defect is already live

The caveat cites `ipc/protocol.rs:87-89` for "`handle_command_status` routes PTY
job ids to `UnknownJob`". That is a **doc comment, not routing code** — the same
category of error this report already retracted for `tools.rs:4914`.

`state.rs:201` mints one `Arc<JobManager>`, cloned into CommandRuntime (`:279`),
WatchRuntime (`:300`) and PtyRuntime (`:311`). `JobManager::get`
(`core/src/job.rs:288`) is a plain map lookup with no `source_type` filter. So
`CommandRuntime::status` **finds** PTY jobs; only the private `live` map misses,
and that miss becomes `.unwrap_or_default()` (`command.rs:1858`) — zeros.

Measured against a live `0.1.86` daemon, one PTY job (`hostname`):

| call | result |
|---|---|
| `command_status` | `state exited`, `exit_code 0`, `duration_ms 813`, `frames_total 0`, `bytes_total 0`, `receipt null`, **`restarted false`** |
| `command_output_tail` | `lines ["CRRR65734"]`, `returned_lines 1` |

Same job id. Status reports zero frames and zero bytes; the tail returns the
actual output line. No restart is involved.

This is strictly worse than TC-B3, because `restarted: false` positively asserts
"observed live" — so proposal 1's trust field would **certify the lie** rather
than expose it. It is also a live regression of the exact bug `command_status`'s
own S1 comment (`command.rs:1832-1840`) records as fixed: status reporting
`bytes_total: 0` while `command_output_tail` returned captured lines.

### Reframing

The document frames the defect as a property of restart reconstruction. The PTY
finding shows the broken invariant is more general:

> `CommandStatusResponse` has no way to express "I don't know". Every counter is
> a non-optional `u64`, and `0` is a legal observed value. Every path lacking
> metrics — restart reconstruction (`ipc/handlers/command.rs:232-235`), a live
> cross-lane miss (`command.rs:1858`) — silently emits a confident zero.

Restart reconstruction is one instance; the PTY read is a second, already
shipping. The root fix is making absence representable (`Option<u64>` on the
wire, serde-skipped so existing clients see today's shape), and computing trust
where the metrics lookup succeeds or fails — which is inside `CommandRuntime`,
exactly where this report's own "Related defect" section argues the fix belongs.
Those two sections agree more than the document notices.

### Sequencing

Agreed, with one insertion: **fix Wrong 3 first, alongside proposal 5.**

It is live with no precondition, it is small (reject a non-`Process`
`source_type` from `CommandRuntime::status` with `UnknownJob`, which makes the
existing protocol comment true instead of rewriting it), and proposal 5 cannot
ship without it — a contract saying "`restarted: false` means the counters are
live observations" is a documented falsehood while PTY jobs answer that way.
Proposal 1 would compound it.

Within step 2, fold the "Related defect" in rather than deferring it: build the
reconstruction in `CommandRuntime::status`, leave the IPC handler a pass-through,
and Wrong 3, the embed gap, and the reconstruction shape are all fixed in one
function.

On the explicit scope question — combed lane vs extending receipts to the PTY
waiter — **keep receipt persistence combed-only, but fix the status read path for
all lanes now.** They are separable. PTY receipts are real work with a live
secret-redaction concern (the no-secret-leak argument in `ipc/protocol.rs:87-89`
depends on PTY never producing a receipt tail, and that argument is currently
resting on a false premise). Rejecting cross-lane status reads is small and
closes the shipping bug. Do not couple them.
