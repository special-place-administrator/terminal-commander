-- SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
-- Outcome evidence and lost-detection support (spec 004, FR-002/FR-003/FR-009).
--
-- V0007 made a job receipt a "minimal backstop": enough to answer a post-restart
-- status poll with a terminal state and exit code. Minimal turned out to be
-- indistinguishable from empty -- a genuine pass and a hypothetical lost run
-- presented identically, so agents re-ran work that had already succeeded.
--
-- This migration stores the evidence a live observer would have had, so a
-- reconstructed outcome is USABLE rather than merely labelled.
--
-- `metrics_json` is a small JSON object of BOUNDED NUMERIC AND IDENTIFIER
-- evidence only -- frames, bytes, suppression counts, duration, probe id. It
-- deliberately carries NO frame text: constitution III bars raw frames from
-- persistent output, which is why the bounded no-silence tail is NOT persisted
-- (spec decision D2).
--
-- `end_cause` records WHY a job ended when that is not self-evident from the
-- terminal state. Today the only value is 'abandoned' (ended by daemon shutdown
-- or stale replacement rather than reaching its own conclusion). Abandonment is
-- deliberately NOT a new `JobState` variant (spec decision D1): the lifecycle
-- state stays the truthful 'cancelled' and the cause rides alongside, so the
-- core lifecycle vocabulary shared by every lane is untouched and older clients
-- keep decoding.
--
-- The audit index supports lost-detection: distinguishing a job the daemon
-- durably recorded STARTING but never recorded FINISHING from an id it never
-- knew. V0003 indexes action/decision/timestamp but NOT subject, and audit rows
-- are never pruned, so an unindexed subject lookup scans a table that only
-- grows.
--
-- Additive only: no backfill, no rewrite, no column drop. Rows written before
-- this migration keep NULL evidence and are reported honestly rather than as
-- zeroes-presented-as-observations.

ALTER TABLE job_receipts ADD COLUMN metrics_json TEXT;

ALTER TABLE job_receipts ADD COLUMN end_cause TEXT;

CREATE INDEX IF NOT EXISTS idx_audit_records_action_subject
    ON audit_records(action, subject);
