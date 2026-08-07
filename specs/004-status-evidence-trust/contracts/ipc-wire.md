# Contract: IPC wire changes

All changes are **additive**. Mixed-version pairing is a standing configuration
(stale-replacement replaces only older daemons), so no existing field changes
type, meaning, or presence.

## `CommandStatusResponse`

### Added

```rust
/// How this outcome was established. Present on EVERY response, never
/// conditionally omitted -- the strict-superset rule.
#[serde(default)]
pub outcome_trust: OutcomeTrust,
```

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeTrust {
    #[default]
    Observed,
    Reconstructed,
    Lost,
    Abandoned,
}
```

`#[serde(default)]` plus `#[default] Observed` means a payload produced by an
older daemon decodes unchanged against the new struct.

### Unchanged, deliberately

- `frames_total`, `frames_stdout`, `frames_stderr`, `bytes_total`,
  `events_emitted` remain bare `u64`. **Optionality is rejected** — those fields
  carry no `#[serde(default)]`, so omitting them breaks decode on every client
  built against the current schema, in exactly the case the change would exist to
  serve. See research R4.
- `restarted` keeps its type, its `#[serde(default)]`, and its meaning. It
  becomes a derived alias: `restarted == (outcome_trust == Reconstructed)`.
- `exit_code` stays truthful when recovered from the durable record. It is
  **not** forced to null on reconstruction (spec FR-008).

### Semantics now guaranteed

| Guarantee | Enforced by |
|---|---|
| A counter is never reported unless observed or durably recorded | FR-001 |
| `observed` implies every counter is a live observation | FR-006, lane routing |
| A finished job's state, exit code and duration stay readable in every lane | FR-005 |
| Abandonment never presents as failure | FR-014, D1 |

## Error codes

### Added

`JobLost` — the engine durably recorded this job starting and never recorded it
finishing.

Chosen as an error code rather than a lifecycle state because "the daemon lost the
thread" is not a job lifecycle state, and because an unknown error code fails
closed for older clients, whereas an unknown enum variant in a core type may fail
open or fail to decode.

### Unchanged

`UnknownJob` — no durable record of this identifier at all. Narrower than today:
it no longer absorbs the lost case.

**Contract change for internal callers**: three call sites currently treat
`UnknownJob` as the only "not found" outcome (`ipc/server.rs:1045`,
`ipc/handlers/runtime.rs:17-18`, `subscriptions/pull.rs:428-429`). All three were
verified unable to pass a cross-lane or post-restart id, but each must be
re-read against the widened error surface.

## `QuiesceForReplace` (new request)

```text
Request : QuiesceForReplace
Response: QuiesceAck { recorded: u32 }
```

Asks a daemon being replaced to durably record its in-flight jobs as abandoned
before it is killed.

**Authorization**: mirrors `Shutdown` exactly — no `[policy.caps]` flag,
protected by the local-only endpoint and attested peer identity. `Shutdown` is
dispatched with no policy gate (`crates/daemon/src/ipc/server.rs:854-857`), and
`QuiesceForReplace` is strictly weaker: it spawns nothing, kills nothing, and
only writes records about the daemon's own jobs. Adding a capability flag would
gate a capability that does not exist.

**Bounded and non-blocking**: the replacer waits a bounded interval. Timeout,
error, or an old daemon that does not know the verb all fall back to current
behaviour — the replacement proceeds. Quiescing must never be able to block an
upgrade.

## Non-changes

- No transport change. No listener. Principle IV untouched.
- No MCP tool added or removed, so every tool-count anchor and the
  `system_discover` fixture stay valid.
- `command_output_tail` keeps answering cross-lane. It returns *correct* data
  today; only `command_status` misreports. A blanket lane guard would remove a
  working capability.
