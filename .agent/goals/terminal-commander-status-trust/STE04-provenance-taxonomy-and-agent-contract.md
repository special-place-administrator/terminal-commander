---
goal_id: STE04
title: Provenance Taxonomy And Agent Contract
chain_id: terminal-commander-status-trust
phase: Wave 3 - Trust
status: "Pending"
depends_on: [STE02]
target_branch: "004-status-evidence-trust"
prohibited_branches: ["main", "master", "production", "release"]
worktree_hint: ""
created_at: "2026-08-06T00:00:00+00:00"
started_at: ""
completed_at: ""
completion_commit: ""
blocked_reason: ""
source_refs:
  - "specs/004-status-evidence-trust/spec.md (requirements)"
  - "specs/004-status-evidence-trust/plan.md (Constitution Check)"
  - "specs/004-status-evidence-trust/research.md (verified findings)"
  - "specs/004-status-evidence-trust/tasks.md (T026-T032, T059)"
  - "docs/audits/2026-08-06-orphaned-job-reports-exit-0.md (source defect report)"
  - "docs/reviews/2026-08-06-tc-b3-adversarial-review.md (remedy corrections)"
risk_level: "medium"
---

# STE04 - Provenance Taxonomy And Agent Contract

## Branch Guard

```text
004-status-evidence-trust
```

## Mission Context

Carry provenance in one closed enum on every response and write the agent-facing contract that does not exist today.

Authoritative detail lives in `specs/004-status-evidence-trust/`. This goal file
is the chain entry, not a second copy of the spec; where the two differ, the spec
wins.

## Mini-Spec

objective:
- Complete tasks T026-T032, T059 in `specs/004-status-evidence-trust/tasks.md`.

non_goals:
- Do not start tasks belonging to another goal in this chain.
- Do not push to `main` or merge without explicit human approval.

allowed_files_or_area:
- `crates/ipc/src/protocol.rs`
- `crates/mcp/src/tools.rs`
- `docs/mcp/**`
- `crates/daemon/tests/**`

forbidden_files:
- secrets, tokens, private usernames, private absolute paths
- any file outside the allowed area above

invariants:
- MUST follow STE02 - an 'observed' indicator on a cross-lane read would certify a falsehood.
- outcome_trust is present on EVERY response, never conditionally omitted.
- No item may be deferred to reduce work. If an item is needed for the plan to
  succeed it is implemented here.

acceptance_criteria:
- Every listed task is complete and its acceptance test passes.
- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`
  and `cargo nextest run --workspace` are green (Constitution VI minimum).

evidence_required:
- Branch evidence (`git branch --show-current`).
- Files changed.
- PASS/FAIL per verification command.
- Source-status label for every behavior touched.

stop_conditions:
- Branch is not `004-status-evidence-trust`.
- A forbidden-file diff appears.
- An invariant above cannot be honoured without changing the spec.

verification_command:
```bash
git branch --show-current
git status --short
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
```

## Task Prompt

Run STE04 only on branch `004-status-evidence-trust`. Route every build and test through Terminal
Commander `run_and_watch`; never `| tail`, never sleep-poll. Stop on any
forbidden-file diff.

## Final Report Format

Objective / Changes / Files changed / Verification / Evidence / Source-status
notes / Commit / Known gaps / Next goal.
