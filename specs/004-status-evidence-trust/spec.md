# Feature Specification: Status Evidence and Trust

**Feature Branch**: `004-status-evidence-trust`

**Created**: 2026-08-06

**Status**: Draft

**Input**: User description: "Restore evidence and trust to command_status so a terminal outcome is never presented without the evidence that justifies it"

## Context

Terminal Commander's value proposition is that an agent can trust what the engine
reports. A status lookup currently returns outcomes whose supporting evidence has
been silently discarded, and returns them in a shape indistinguishable from a
fully-observed result. A field user acted on that ambiguity, discarded two
genuinely passing test suites, and re-ran them — about 40 minutes lost.

Two distinct paths produce the same misleading shape:

1. A job whose in-memory record is gone but whose outcome survived in durable
   storage. The outcome is true; every corroborating count reads as zero.
2. A job belonging to an interactive or file-watch lane. The outcome is true;
   the counts read as zero **and** the response asserts they were observed live.

Supporting analysis: `docs/audits/2026-08-06-orphaned-job-reports-exit-0.md`.
Adversarial review of the remediation plan:
`docs/reviews/2026-08-06-tc-b3-adversarial-review.md`.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - An agent can bank a reconstructed pass without re-running (Priority: P1)

An agent starts a long test suite through the harness. The suite finishes
successfully. Before the agent polls for the result, the engine is replaced or
restarted. The agent polls, and receives not only the true outcome but the
evidence that justifies it — how much output the job produced, how long it ran,
and which probe produced it. The agent accepts the pass and moves on.

**Why this priority**: This is the reported loss. Labelling alone does not fix
it: any harness gating expensive work will re-run rather than bank an
unverifiable pass. Evidence is what removes the re-run.

**Independent Test**: Run a command to completion, capture its live result,
restart the engine, poll again, and confirm the second answer carries the same
evidence as the first.

**Acceptance Scenarios**:

1. **Given** a command that ran to completion and produced output, **When** the
   engine is restarted and its status is polled, **Then** the response reports
   the same output volume, duration, and originating probe as the pre-restart
   response.
2. **Given** that same reconstructed response, **When** an agent inspects it,
   **Then** the response states plainly that it was reconstructed from durable
   storage rather than observed live.
3. **Given** a command that produced no rule matches, **When** its status is
   polled after a restart, **Then** the bounded no-silence tail is still
   available.

---

### User Story 2 - Status never reports counts it did not observe (Priority: P1)

An agent starts an interactive (PTY) job, then asks for that job's status. The
response either reports that job's real counts, or declines to answer — but never
reports zero counts while asserting they were observed.

**Why this priority**: This defect is live today with no precondition, and it is
the more dangerous of the two because the response positively claims the counts
are live observations. Any trust signal added before this is fixed would certify
a falsehood.

**Independent Test**: Start an interactive job that produces output, poll its
status, and confirm the reported output volume matches what the output tail
returns for the same job.

**Acceptance Scenarios**:

1. **Given** an interactive job that has produced output, **When** its status is
   polled, **Then** the reported output volume is non-zero and matches the
   output actually captured for that job.
2. **Given** a finished interactive job, **When** its status is polled, **Then**
   its terminal state, exit code, and duration remain readable.
3. **Given** a file-watch job, **When** its status is polled, **Then** the same
   guarantees hold.

---

### User Story 3 - An agent can tell "observed", "reconstructed", "lost", and "unknown" apart (Priority: P2)

An agent receives any status response and can determine, from one obvious field,
how much the engine actually witnessed — without inferring it from a conspiracy
of zeroed counters.

**Why this priority**: Prevents over-trust once Story 1 makes reconstructed
results usable. Depends on Story 4 for its third value to be emittable honestly.

**Independent Test**: Produce each of the four conditions and confirm each yields
a distinct, documented answer.

**Acceptance Scenarios**:

1. **Given** a job observed end to end, **When** its status is polled, **Then**
   the trust indicator reads "observed".
2. **Given** a job reconstructed from durable storage, **When** its status is
   polled, **Then** the trust indicator reads "reconstructed".
3. **Given** an identifier the engine has no record of ever starting, **When**
   its status is polled, **Then** the response is an unknown-identifier error,
   distinct from the lost case.
4. **Given** an existing consumer that only understands the previous
   restart-marker field, **When** it reads a new response, **Then** it still
   decodes successfully and the marker still carries its original meaning.

---

### User Story 4 - A job the engine started but never finished is provably distinguishable from one it never knew (Priority: P2)

An agent polls for a job that was started but whose completion was never
recorded, because the engine died mid-run. The response says so, rather than
implying the identifier was never valid.

**Why this priority**: Turns an ambiguous error into a diagnosis, and supplies
Story 3's third value. The durable start record needed for this already exists.

**Independent Test**: Start a long job, terminate the engine abruptly, restart,
and poll — then compare against polling a fabricated identifier.

**Acceptance Scenarios**:

1. **Given** a job with a durable start record and no recorded outcome, **When**
   its status is polled, **Then** the response identifies it as lost.
2. **Given** an identifier with no durable record at all, **When** its status is
   polled, **Then** the response identifies it as unknown.
3. **Given** a lost job, **When** its status is polled, **Then** the response
   never reports a successful terminal outcome.

---

### User Story 5 - A planned engine replacement does not silently orphan in-flight work (Priority: P3)

An operator upgrades the product while work is in flight. The replaced engine
records that its unfinished jobs were abandoned, so a later poll reports
abandonment rather than silence or a false failure.

**Why this priority**: Closes the exact trigger of the reported incident — a
version-stale engine being replaced underneath running jobs. Lower priority
because Story 4 already detects the condition after the fact.

**Independent Test**: Start a long job, trigger a graceful shutdown and then a
stale-replacement, restart, and poll.

**Acceptance Scenarios**:

1. **Given** an in-flight job and a graceful shutdown, **When** status is polled
   after restart, **Then** the response reports abandonment with no exit code.
2. **Given** an in-flight job and a stale-engine replacement, **When** status is
   polled after restart, **Then** the response reports abandonment rather than a
   bare unknown identifier.
3. **Given** an abandoned job, **When** its status is polled, **Then** it is
   never reported as a failure, because it did not fail.

---

### User Story 6 - Both delivery modes answer identically (Priority: P3)

A host embedding the engine directly receives the same status answers as a host
talking to it over the adapter, including reconstructed outcomes.

**Why this priority**: The engine already writes the durable record on both
paths; only one path can read it back. Constitutionally, both delivery shapes
must terminate at the same authority.

**Independent Test**: Exercise the same reconstruction through the embedded
surface and confirm answers match.

**Acceptance Scenarios**:

1. **Given** a durable outcome record, **When** an embedding host asks the engine
   for that job's status, **Then** it receives the same reconstructed answer the
   adapter surface returns.

---

### Edge Cases

- A job forcibly stopped by the operator: its recorded event count must reflect
  that no completion event was appended, and must not inherit the natural-exit
  adjustment.
- A job whose completion event could not be appended: the recorded count must not
  claim an event that does not exist.
- A durable outcome record written before this feature exists: it carries no
  evidence and must be reported honestly rather than as zeroes-as-observations.
- A start record whose durable write silently failed: the lost/unknown
  classification degrades toward "unknown", never toward a false terminal.
- An interactive job that is still running: status must not report a terminal
  state or fabricate counts.
- An abandonment record read by a consumer that predates abandonment: it must not
  decode as a successful or failed outcome.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: A status response MUST NOT report a count, duration, or volume the
  engine did not actually observe or durably record.
- **FR-002**: When a job's live record is absent but its outcome is durably
  recorded, the response MUST include the output volume, duration, and
  originating probe captured at the time the job finished.
- **FR-003**: The durable outcome record MUST capture that evidence at the moment
  the outcome becomes final, and MUST record an event count consistent with what
  a live observer of the same run would have seen.
- **FR-004**: A status request for a job belonging to a lane the responder does
  not own MUST be answered by the owning lane, so real counts are returned; it
  MUST NOT return zeroes attributed to live observation.
- **FR-005**: A finished interactive job's terminal state, exit code, and
  duration MUST remain readable after this change; no currently readable outcome
  may become unreadable.
- **FR-006**: Every status response MUST carry a single explicit indicator of how
  the outcome was established: observed, reconstructed, or lost.
- **FR-007**: The previously shipped restart marker MUST continue to decode and
  retain its meaning for existing consumers; the change MUST be additive.
- **FR-008**: A truthful exit code recovered from durable storage MUST be
  preserved, not suppressed. Only genuinely unobserved outcomes may report no
  exit code.
- **FR-009**: The engine MUST distinguish a job it durably recorded starting but
  never recorded finishing, from an identifier it has no record of.
- **FR-010**: Lost-detection MUST cover every lane that durably records a start,
  or MUST explicitly document which lanes it covers.
- **FR-011**: Lost-detection MUST fail safe: an inconclusive lookup reports
  unknown, never a terminal outcome.
- **FR-012**: On a graceful shutdown, the engine MUST record its unfinished jobs
  as abandoned before durable storage closes.
- **FR-013**: Before an engine is replaced for being version-stale, the outgoing
  engine MUST be given a bounded opportunity to record its unfinished jobs as
  abandoned; failure to do so MUST fall back to current behaviour, not block the
  replacement.
- **FR-014**: An abandoned outcome MUST be reported as abandoned and MUST NOT be
  reported as a failure or a success.
- **FR-015**: The reconstruction capability MUST be reachable through the engine's
  own typed interface, so both delivery modes behave identically.
- **FR-016**: The agent-facing description of the status operation MUST document
  the trust indicator and what each value implies; it currently documents none.
- **FR-017**: Documentation and inline contracts that this change falsifies MUST
  be corrected in the same change.
- **FR-018**: Any recovery guidance that instructs an agent to confirm state via a
  status lookup MUST remain valid for every lane after this change.

### Key Entities

- **Job Outcome Record**: The durable record of how a job ended. Today holds the
  terminal state, exit code, and a rule-driven event count. Must additionally
  hold the evidence a live observer would have had.
- **Job Start Record**: The durable record that a job began. Already written for
  every lane; currently unused for diagnosis.
- **Trust Indicator**: How the reported outcome was established.
- **Abandonment Record**: A durable statement that a job was terminated by engine
  shutdown or replacement rather than reaching its own conclusion.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: An agent receiving a reconstructed outcome for a job that produced
  output can see non-zero output volume and a duration, and therefore needs zero
  re-runs to accept the result. Target: the reported incident's 40-minute loss
  becomes zero.
- **SC-002**: For every job the engine can answer about, the output volume it
  reports in a status lookup equals the output volume observable through the
  output tail for that same job. No lane reports zero while output exists.
- **SC-003**: 100% of status responses carry a trust indicator, and each of its
  values is reachable by a reproducible scenario.
- **SC-004**: A consumer built against the previous response shape continues to
  decode every response this feature produces, with zero decode failures across
  a mixed-version pairing.
- **SC-005**: For a job started and then lost to abrupt engine death, the
  diagnosis distinguishes it from an unrecognised identifier in 100% of cases
  where a durable start record exists.
- **SC-006**: No status response reports a successful terminal outcome for a job
  whose completion was never observed — verified by a repeatable abrupt-kill
  scenario.
- **SC-007**: An abandoned job is never reported as failed.
- **SC-008**: The embedded delivery mode and the adapter delivery mode return
  equivalent answers for the same job in every scenario above.

## Assumptions

- The consumer population is agent harnesses and operators using the product's
  tool surface; there is no human GUI to consider.
- Mixed-version pairings occur in practice: replacement logic only replaces
  *older* engines, so an older client talking to a newer engine is a standing
  configuration. Response-shape changes must therefore be additive only.
- Durable records are not pruned today, so historical start records remain
  available for diagnosis without a retention policy change.
- Outcome records written before this feature will lack evidence permanently;
  they are reported honestly rather than backfilled.
- Abrupt engine death caused by the operating system or an external kill remains
  unrecordable at the moment it happens; detection after the fact is the intended
  mitigation.
- The reported incident's trigger — replacing a version-stale engine while work
  is in flight — is the specific sequence Story 5 must cover.

## Outstanding Clarifications

- **[NEEDS CLARIFICATION: abandonment representation]** An abandoned outcome has
  no home in the current lifecycle vocabulary, and an unrecognised label is
  currently coerced into "failed" — which would manufacture exactly the false
  negative this feature exists to remove. Options: introduce abandonment as a
  first-class lifecycle state, or carry it as a separate attribute alongside the
  existing states. This determines whether Story 5 is additive or touches a core
  vocabulary shared by every lane.

- **[NEEDS CLARIFICATION: durable retention of raw output tail]** Story 1's third
  acceptance scenario requires the bounded no-silence tail to survive a restart,
  which extends the lifetime of raw captured output from memory into durable
  storage. Constitution III sanctions that tail on the wire but is silent on
  persisting it. Options: persist it, persist it with redaction, or drop that
  scenario and keep the tail memory-only. This is a governance decision, not a
  technical one.
