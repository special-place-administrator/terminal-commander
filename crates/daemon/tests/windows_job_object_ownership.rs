// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
// Copyright 2026 The Terminal Commander Authors

//! spec 004 T5 / review tripwire: a child killed by `KILL_ON_JOB_CLOSE` must
//! never persist `terminal_state: "exited", exit_code: 0`.
//!
//! ## Why this is a static tripwire and not a fault-injected behavioural test
//!
//! The concern is real: Windows children run inside a Job Object with
//! `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`. If any ordering let the job handle
//! close while a waiter was still alive to observe `probe.wait()`, that waiter
//! could see a plausible `Ok(status)` for a child that was actually killed, and
//! persist a receipt claiming `exited`.
//!
//! It cannot happen, and the reason is OWNERSHIP rather than sequencing:
//!
//! 1. `ProcessProbe` holds `_job: Option<Arc<JobHandle>>`.
//! 2. `KILL_ON_JOB_CLOSE` fires only when the LAST `Arc` drops, via
//!    `JobHandle::Drop -> CloseHandle`.
//! 3. `drive_to_exit` takes the probe **by value** and holds it across
//!    `probe.wait().await`.
//!
//! So for the whole duration of the wait, the waiter itself owns a live `Arc`
//! to the job handle. The handle cannot close underneath the very task that
//! would observe and persist the result.
//!
//! Forcing the failure would require production code to expose a seam that only
//! a test uses, which constitution VI (NON-NEGOTIABLE) forbids: "Production code
//! paths MUST NOT reach into test-only logic." `CONTRIBUTING.md` §6.1 sanctions
//! exactly this fallback -- "record the ownership argument as a documented
//! invariant with a review tripwire" -- for Windows `cfg` sentinels headless CI
//! cannot exercise live.
//!
//! These assertions therefore guard the three structural facts the argument
//! rests on. If a refactor breaks any of them the argument no longer holds, and
//! this test fails loudly rather than the guarantee silently evaporating.
//!
//! Source-status: `partial` -- structural guard, not live fault injection.

#![cfg(windows)]

fn read_repo_file(rel: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// Fact 1 + 2: the probe owns a shared handle whose `Drop` closes the job.
#[test]
fn process_probe_still_owns_the_job_handle_arc() {
    let source = read_repo_file("../probes/src/process.rs");

    assert!(
        source.contains("_job: Option<Arc<JobHandle>>"),
        "ProcessProbe must still hold `_job: Option<Arc<JobHandle>>`. If the \
         probe stops owning an Arc to the job handle, the waiter no longer keeps \
         the job alive across `wait()`, and KILL_ON_JOB_CLOSE could fire while a \
         waiter is still able to observe and persist a plausible exit status."
    );
    assert!(
        source.contains("impl Drop for JobHandle") && source.contains("CloseHandle"),
        "JobHandle::Drop -> CloseHandle is the mechanism that makes \
         KILL_ON_JOB_CLOSE last-Arc-scoped. Removing it changes when the child \
         tree dies relative to the waiter."
    );
}

/// Fact 3: the waiter takes the probe BY VALUE, so it owns an Arc for the whole
/// wait. A `&mut ProcessProbe` signature would let the caller drop the probe --
/// and the last Arc -- while the wait is still in flight.
#[test]
fn drive_to_exit_still_takes_the_probe_by_value() {
    let source = read_repo_file("src/command.rs");

    assert!(
        source.contains("async fn drive_to_exit(mut probe: ProcessProbe)"),
        "drive_to_exit MUST take `mut probe: ProcessProbe` BY VALUE. Taking it \
         by reference would let the owner drop the probe (and the last \
         Arc<JobHandle>) while `probe.wait()` is still in flight, which is the \
         one ordering that could surface a killed child as a clean exit."
    );
    assert!(
        source.contains("probe.wait().await"),
        "drive_to_exit must still await `probe.wait()` while holding the probe; \
         the ownership argument is about what is alive ACROSS that await."
    );
}

/// The complementary branch: an outcome with no reaped status must not become a
/// success. `Cancelled` carries no exit code.
#[test]
fn a_cancelled_outcome_still_maps_to_no_exit_code() {
    let source = read_repo_file("src/command.rs");

    assert!(
        source.contains("ProbeOutcome::Cancelled => None"),
        "A cancelled probe MUST map to `exit_code: None`. Mapping it to Some(0) \
         -- or letting it fall through to a default -- would let an explicit \
         kill surface as a clean exit, which is the same false-green this \
         tripwire exists to prevent."
    );
}
