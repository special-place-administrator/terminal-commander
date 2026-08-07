// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
// Copyright 2026 The Terminal Commander Authors

//! spec 004 US5 / FR-012 / FR-014 / decision D1: a job ended BY a daemon
//! shutdown must be reported as abandoned -- never as a failure, and never as
//! silence.
//!
//! D1 exists because of a trap adversarial review caught: the reconstruction
//! maps any unrecognised terminal label to `JobState::Failed`, and `JobState`
//! has no `Abandoned` variant. Writing `terminal_state: "abandoned"` would
//! therefore have surfaced abandoned jobs as FAILED -- manufacturing exactly the
//! false negative this feature exists to remove. So abandonment rides
//! `end_cause` + `outcome_trust` while the lifecycle state stays the truthful
//! `Cancelled`.

use std::path::PathBuf;

use terminal_commander_core::{BucketId, JobConfig, JobId, JobState, ProbeId, SourceType};
use terminal_commanderd::{DaemonConfig, DaemonState, OutcomeTrust};

fn tmp_data_dir(tag: &str) -> PathBuf {
    static TC_DD_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let mut p = std::env::temp_dir();
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let n = TC_DD_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    p.push(format!("tc-abandon-{tag}-{pid}-{nanos}-{n}"));
    p
}

fn cleanup(p: &std::path::Path) {
    let _ = std::fs::remove_dir_all(p);
}

fn register_running_job(state: &DaemonState) -> JobId {
    let job_id = JobId::new();
    state.jobs.start(JobConfig {
        job_id,
        argv: vec!["in-flight".to_owned()],
        bucket_id: BucketId::new(),
        probe_id: ProbeId::new(),
        source_type: SourceType::Process,
        grace_secs: 0,
    });
    state.jobs.mark_running(job_id);
    job_id
}

#[test]
fn an_in_flight_job_is_recorded_as_abandoned_and_never_as_failed() {
    let data = tmp_data_dir("in-flight");
    let state = DaemonState::bootstrap(DaemonConfig::defaults_in(&data)).unwrap();

    let job_id = register_running_job(&state);
    state.record_abandoned_jobs();

    let recon = state
        .command
        .reconstructed_status(job_id)
        .expect("an abandoned job must leave a durable receipt");

    assert_eq!(
        recon.outcome_trust,
        OutcomeTrust::Abandoned,
        "the trust indicator carries the cause"
    );
    assert_ne!(
        recon.state,
        JobState::Failed,
        "an abandoned job did NOT fail -- reporting it as failed is the false \
         negative decision D1 exists to prevent"
    );
    assert_eq!(
        recon.state,
        JobState::Cancelled,
        "the lifecycle state stays truthful: the job was terminated"
    );
    assert_eq!(
        recon.exit_code, None,
        "an abandoned job produced no exit status; one must never be invented"
    );
    assert!(
        recon.restarted,
        "`restarted` is the compat alias for 'not observed live', so an older \
         client still sees that this was not a live observation"
    );

    cleanup(&data);
}

#[test]
fn a_job_that_finished_on_its_own_is_not_marked_abandoned() {
    // Only jobs still non-terminal at shutdown were ended BY the shutdown. A
    // job that reached its own conclusion keeps that conclusion.
    let data = tmp_data_dir("finished");
    let state = DaemonState::bootstrap(DaemonConfig::defaults_in(&data)).unwrap();

    let job_id = register_running_job(&state);
    let _ = state.jobs.finish(job_id, Some(0), None);

    state.record_abandoned_jobs();

    assert!(
        state.command.reconstructed_status(job_id).is_none(),
        "a job that already reached a terminal state must not get an \
         abandonment receipt written over it at shutdown"
    );

    cleanup(&data);
}

#[test]
fn recording_abandonment_is_safe_with_no_jobs_in_flight() {
    // Shutdown must never be blockable by bookkeeping, including the empty case.
    let data = tmp_data_dir("empty");
    let state = DaemonState::bootstrap(DaemonConfig::defaults_in(&data)).unwrap();
    state.record_abandoned_jobs();
    cleanup(&data);
}
