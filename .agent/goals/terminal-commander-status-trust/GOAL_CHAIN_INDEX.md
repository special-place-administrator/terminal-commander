# Goal Chain Index - terminal-commander-status-trust

Target branch: `004-status-evidence-trust`

Supersedes nothing. The `terminal-commander-mvp` chain is closed (see its
`FINAL_REPORT.md`); `CONTRIBUTING.md` section 7 still names that closed chain,
which is stale — the live convention is one chain per program, as
`terminal-commander-runtime`, `-npm-distribution` and `-windows-wsl-bridge`
already demonstrate.

Authoritative requirements live in `specs/004-status-evidence-trust/`. This chain
indexes execution; it does not duplicate the spec.

| Goal | Title | Status | Depends on | Branch | Intended outcome |
|---|---|---|---|---|---|
| STE01 | Outcome Evidence Foundation | Pending | [] | `004-status-evidence-trust` | Add the V0008 additive migration, the receipt evidence read/write, the subject-filtered audit read, and the wire vocabulary (`OutcomeTrust`, `outcome_trust`, `JobLost`). |
| STE02 | Lane Ownership For Status Reads | Pending | [STE01] | `004-status-evidence-trust` | Route a status read to the runtime that owns the job so PTY and watch lanes return real counters. |
| STE03 | Reconstructed Outcome Evidence | Pending | [STE01] | `004-status-evidence-trust` | Persist the evidence a live observer would have had and populate the reconstruction from it, so a restart-reconstructed pass is usable rather than merely labelled. |
| STE04 | Provenance Taxonomy And Agent Contract | Pending | [STE02] | `004-status-evidence-trust` | Carry provenance in one closed enum on every response and write the agent-facing contract that does not exist today. |
| STE05 | Lost Detection | Pending | [STE01] | `004-status-evidence-trust` | Distinguish a job the engine recorded starting but never recorded finishing from an identifier it never knew, using the per-lane start records that already exist. |
| STE06 | Abandonment Records | Pending | [STE04, STE05] | `004-status-evidence-trust` | Record abandonment at graceful shutdown and before stale replacement, so a planned death is accounted for rather than silent. |
| STE07 | Delivery Mode Parity | Pending | [STE03] | `004-status-evidence-trust` | Move reconstruction into the engine's typed status API so the embedded delivery mode reaches it, per docs/EMBEDDING. |
| STE08 | Contract Sweep And Verification Gate | Pending | [STE06, STE07] | `004-status-evidence-trust` | Correct every inline contract this feature falsifies, run both OS gates, capture dogfood evidence, and open the PR. |

## Governance

- Constitution Check: PASS, no violations (`specs/004-status-evidence-trust/plan.md`).
- Two exclusions are recorded governance decisions, not deferrals: durable raw-tail
  persistence (Constitution III) and proposal 2 (merged into STE03).
- Branch protection requires a pull request; direct pushes to `main` bypass 8
  required status checks.
