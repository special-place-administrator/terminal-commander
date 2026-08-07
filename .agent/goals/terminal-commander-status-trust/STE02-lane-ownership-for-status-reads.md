---
goal_id: STE02
title: Lane Ownership For Status Reads
chain_id: terminal-commander-status-trust
phase: Wave 1 - Live Defect
status: "Pending"
depends_on: [STE01]
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
  - "specs/004-status-evidence-trust/tasks.md (T010-T017, T057)"
  - "docs/audits/2026-08-06-orphaned-job-reports-exit-0.md (source defect report)"
  - "docs/reviews/2026-08-06-tc-b3-adversarial-review.md (remedy corrections)"
risk_level: "high"
---

# STE02 - Lane Ownership For Status Reads

## Branch Guard

```text
004-status-evidence-trust
```

## Mission Context

Route a status read to the runtime that owns the job so PTY and watch lanes return real counters. This is the defect that is live today with no precondition.

Authoritative detail lives in `specs/004-status-evidence-trust/`. This goal file
is the chain entry, not a second copy of the spec; where the two differ, the spec
wins.

## Mini-Spec

objective:
- Complete tasks T010-T017, T057 in `specs/004-status-evidence-trust/tasks.md`.

non_goals:
- Do not start tasks belonging to another goal in this chain.
- Do not push to `main` or merge without explicit human approval.

allowed_files_or_area:
- `crates/daemon/src/command.rs`
- `crates/daemon/src/pty_command.rs`
- `crates/daemon/src/file_watch.rs`
- `crates/daemon/src/ipc/handlers/command.rs`
- `crates/daemon/tests/**`

forbidden_files:
- secrets, tokens, private usernames, private absolute paths
- any file outside the allowed area above

invariants:
- MUST NOT answer a live job with a bare UnknownJob - Constitution VII.
- MUST NOT break command_output_tail, which answers cross-lane correctly today.
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

Run STE02 only on branch `004-status-evidence-trust`. Route every build and test through Terminal
Commander `run_and_watch`; never `| tail`, never sleep-poll. Stop on any
forbidden-file diff.

## Final Report Format

Objective / Changes / Files changed / Verification / Evidence / Source-status
notes / Commit / Known gaps / Next goal.
