---
goal_id: STE03
title: Reconstructed Outcome Evidence
chain_id: terminal-commander-status-trust
phase: Wave 2 - Reported Loss
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
  - "specs/004-status-evidence-trust/tasks.md (T018-T025, T058)"
  - "docs/audits/2026-08-06-orphaned-job-reports-exit-0.md (source defect report)"
  - "docs/reviews/2026-08-06-tc-b3-adversarial-review.md (remedy corrections)"
risk_level: "high"
---

# STE03 - Reconstructed Outcome Evidence

## Branch Guard

```text
004-status-evidence-trust
```

## Mission Context

Persist the evidence a live observer would have had and populate the reconstruction from it, so a restart-reconstructed pass is usable rather than merely labelled. This is the item that recovers the reported 40 minutes.

Authoritative detail lives in `specs/004-status-evidence-trust/`. This goal file
is the chain entry, not a second copy of the spec; where the two differ, the spec
wins.

## Mini-Spec

objective:
- Complete tasks T018-T025, T058 in `specs/004-status-evidence-trust/tasks.md`.

non_goals:
- Do not start tasks belonging to another goal in this chain.
- Do not push to `main` or merge without explicit human approval.

allowed_files_or_area:
- `crates/daemon/src/command.rs`
- `crates/daemon/src/pty_command.rs`
- `crates/daemon/src/ipc/handlers/command.rs`
- `crates/daemon/tests/**`
- `crates/mcp/tests/**`

forbidden_files:
- secrets, tokens, private usernames, private absolute paths
- any file outside the allowed area above

invariants:
- Apply the event-count conjunction in full: move the natural-exit persist below the append AND pass the post-append value. Half the fix is a bug (research R1).
- Leave rule_driven_events untouched - it gates the no-silence receipt.
- Never force exit_code to null (FR-008).
- Persist metrics and identifiers only - no frame text (Constitution III, decision D2).
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

Run STE03 only on branch `004-status-evidence-trust`. Route every build and test through Terminal
Commander `run_and_watch`; never `| tail`, never sleep-poll. Stop on any
forbidden-file diff.

## Final Report Format

Objective / Changes / Files changed / Verification / Evidence / Source-status
notes / Commit / Known gaps / Next goal.
