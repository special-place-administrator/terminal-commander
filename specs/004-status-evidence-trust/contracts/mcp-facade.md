# Contract: MCP agent-facing surface

No tool is added or removed. Every tool-count anchor and the `system_discover`
fixture remain valid. The changes are to what agents are *told*, plus one new
response field passed through.

## `command_status` description

**Today** (`crates/mcp/src/tools.rs:1525`) the description never mentions
`restarted` at all. Agents receive a bare `"restarted": true` with zero contract —
so the fix is writing a contract where none exists, not softening a misleading
one.

**Required substance**:

> Returns bounded counters and exit info for a previously started job.
> `outcome_trust` tells you how the engine knows: `observed` (witnessed live —
> counters are real observations), `reconstructed` (read from the durable record
> after a restart — `state`/`exit_code` are truthful and counters are the values
> captured when the job finished), `lost` (the engine recorded starting this job
> and never recorded it finishing — never treat as success), `abandoned` (ended
> by engine shutdown or replacement, not by the job itself; reported as cancelled
> with no exit code, and is NOT a failure). Never returns raw stream text, with
> one exception: when the command finished and ZERO rules matched, a bounded exit
> receipt is included so a no-rule command is never silent. That receipt is
> memory-only and does not survive a restart — its absence after a restart does
> not mean the command produced no output.

The last sentence is load-bearing: it is what stops decision D2 from recreating
the original defect in a new place.

**Do not** write the earlier draft's wording ("live counters and the receipt are
not retained (zero/null)"). Persisted evidence falsifies it. Write the contract
around provenance, not around zeros.

## `run_and_watch` description

Add `outcome_trust` to the documented result keys alongside the existing
`degraded` / `recover_hint` / `wait_exhausted` vocabulary, and state that it
follows the same strict-superset rule — present on every payload, not only on
reconstructed ones. This mirrors the invariant already pinned by
`run_and_watch_normal_terminal_is_complete_and_a_strict_superset`
(`crates/mcp/src/tools.rs:6345`).

## Transport-failure remedy text

`crates/mcp/src/tools.rs:3700-3702`, pinned by a test at `:7690`, tells agents to
confirm actual state via `command_status` after any mutating operation —
including PTY operations.

Because the status read is **routed** rather than rejected, this advice stays
valid for every lane. Verify the pinned test still passes; no text change is
required unless the wording implies counters are always live observations, in
which case it inherits the `outcome_trust` contract above.

## Payload passthrough

`command_status_payload` (`crates/mcp/src/tools.rs:4894`) gains `outcome_trust`.
The stale `//` comment at `:4914-4918` — which describes counters as zero after a
restart — becomes false once evidence is persisted and must be corrected in the
same change. Note this comment is invisible to agents; correcting it is hygiene,
not the contract fix.

## Documentation

- `docs/mcp/OMNI_PLAYBOOK.md`: the interactive/REPL section must state that a
  finished PTY job's status is readable via `command_status` and that its
  counters are real.
- Any doc asserting that a restart zeroes counters must be corrected.

## Inline contracts falsified by this change

Both are in `crates/ipc/src/protocol.rs` and must be corrected in the same change
(spec FR-017):

- `:128-134` states counters "are zero because the in-memory probe metrics did
  not survive". False once evidence is persisted.
- `:87-89` claims `handle_command_status` routes PTY job ids to `UnknownJob`.
  That routing never existed — it is the defect this feature fixes. The comment
  also names `CommandService`, a type that does not exist. Reword to describe the
  ownership guard that will actually exist.
