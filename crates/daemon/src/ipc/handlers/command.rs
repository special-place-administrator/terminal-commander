// SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0
// Copyright 2026 The Terminal Commander Authors

use std::sync::Arc;

use terminal_commander_supervisor::identity::PeerIdentity;

use super::common::{identity_audit_subject, map_command_error};
use crate::command::CommandStartRequest;
use crate::ipc::protocol::{
    CommandOutputTailParams, CommandOutputTailResponse, CommandStartParams, CommandStatusParams,
    CommandStopParams, CommandStopResponse, IpcError, IpcErrorCode, IpcResponse,
    MAX_COMMAND_ENV_ITEMS, MAX_COMMAND_INLINE_RULES, MAX_TAIL_BYTES, MAX_TAIL_LINES,
    ShellExecParams,
};
use crate::shell::ShellExecRequest;
use crate::state::DaemonState;

pub(in crate::ipc::server) fn handle_command_start_combed(
    state: &Arc<DaemonState>,
    params: &CommandStartParams,
    peer: &PeerIdentity,
) -> Result<IpcResponse, IpcError> {
    if params.env.len() > MAX_COMMAND_ENV_ITEMS {
        return Err(IpcError::new(
            IpcErrorCode::ArgvInvalid,
            format!("env entries {} exceed cap", params.env.len()),
        ));
    }
    if params.rules.len() > MAX_COMMAND_INLINE_RULES {
        return Err(IpcError::new(
            IpcErrorCode::ArgvInvalid,
            format!("inline rules {} exceed cap", params.rules.len()),
        ));
    }
    let req = CommandStartRequest {
        argv: params.argv.clone(),
        cwd: params.cwd.clone(),
        env: params.env.clone(),
        bucket_config: params.bucket_config.clone(),
        rules: params.rules.clone(),
        grace: params.grace(),
        tag: params.tag.clone(),
        // TC-B1: thread the strip flag end-to-end (default true at the IPC
        // boundary via serde `default_true`).
        strip_ansi: params.strip_ansi,
        // TC-2: thread the client dedup hint end-to-end. Without this
        // explicit assignment the field is silently dropped at this hand-
        // built conversion (amendment #7).
        dedup_nonce: params.dedup_nonce.clone(),
        // TC-2 peer-scoped fallback: pre-hash the dispatching peer so the
        // nonce-less fingerprint window only collapses a SAME-peer retry,
        // never a sibling client guessing another peer's command.
        peer_discriminator: Some(peer_discriminator(peer)),
    };
    let resp = state.command.start_combed(req).map_err(map_command_error)?;
    Ok(IpcResponse::CommandStartCombed(resp))
}

/// Handle a `shell_exec` IPC request (TC49). Mirrors
/// [`handle_command_start_combed`] but routes through the gated shell
/// lane: it builds a [`ShellExecRequest`] from the wire params and calls
/// the SYNC [`ShellRuntime::exec`](crate::shell::ShellRuntime::exec),
/// which gates on `PolicyAction::CommandShellStart` (denied by default).
///
/// The shell lane SKIPS the `SHELL_INTERPRETERS_DENY` guard, so it can
/// NEVER produce [`CommandError::ShellInterpreterDenied`]; its denials are
/// [`CommandError::PolicyDenied`], which [`map_command_error`] maps to
/// [`IpcErrorCode::PolicyDenied`]. The reply reuses
/// [`IpcResponse::CommandStartCombed`] — the shell lane returns the same
/// bounded [`CommandStartResponse`](crate::ipc::protocol::CommandStartResponse)
/// shape and never raw stdout/stderr.
///
/// SYNC: `exec` never awaits, so no `.await` here — the async dispatcher
/// calls this inline.
pub(in crate::ipc::server) fn handle_shell_exec(
    state: &Arc<DaemonState>,
    params: &ShellExecParams,
) -> Result<IpcResponse, IpcError> {
    if params.env.len() > MAX_COMMAND_ENV_ITEMS {
        return Err(IpcError::new(
            IpcErrorCode::ArgvInvalid,
            format!("env entries {} exceed cap", params.env.len()),
        ));
    }
    if params.rules.len() > MAX_COMMAND_INLINE_RULES {
        return Err(IpcError::new(
            IpcErrorCode::ArgvInvalid,
            format!("inline rules {} exceed cap", params.rules.len()),
        ));
    }
    let req = ShellExecRequest {
        shell_line: params.shell_line.clone(),
        shell: params.shell.clone(),
        cwd: params.cwd.clone(),
        env: params.env.clone(),
        rules: params.rules.clone(),
        bucket_config: params.bucket_config.clone(),
        tag: params.tag.clone(),
    };
    let resp = state.shell.exec(req).map_err(map_command_error)?;
    Ok(IpcResponse::CommandStartCombed(resp))
}

/// TC-3 `command_stop` handler: force-kill a running combed command.
///
/// Mirrors [`handle_command_start_combed`]'s convention: returns
/// `Result<IpcResponse, IpcError>` and maps the runtime error via
/// [`map_command_error`] (so `PolicyDenied -> PolicyDenied` and
/// `UnknownJob -> UnknownJob` reach the wire with the right codes).
///
/// The peer is rendered to an audit subject via the SHARED
/// [`identity_audit_subject`] helper and passed to `stop` so a
/// policy-denied caller's deny audit row names the PEER, never the
/// `job_id` -- the deny path inside `stop` never touches the live map.
pub(in crate::ipc::server) fn handle_command_stop(
    state: &Arc<DaemonState>,
    params: &CommandStopParams,
    peer: &PeerIdentity,
) -> Result<IpcResponse, IpcError> {
    let peer_subject = identity_audit_subject(peer);
    match state.command.stop(params.job_id, &peer_subject) {
        Ok((bucket_id, m)) => Ok(IpcResponse::CommandStop(CommandStopResponse {
            job_id: params.job_id,
            bucket_id,
            frames_total: m.frames_total,
            events_emitted: m.events_emitted,
            bytes_total: m.bytes_total,
        })),
        Err(e) => Err(map_command_error(e)),
    }
}

/// Stable per-peer discriminator for the TC-2 nonce-less dedup fallback.
///
/// A `DefaultHasher` digest of the peer's stable identity field (uid for
/// Unix, sid for Windows). The pid is deliberately EXCLUDED so two
/// connections from the same principal still dedup a retry. An unknown
/// peer hashes to a single shared bucket -- conservative: it can only
/// collapse with another equally-unknown peer's identical signature
/// inside the short TTL.
fn peer_discriminator(peer: &PeerIdentity) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    match peer {
        PeerIdentity::Unix { uid, .. } => {
            0u8.hash(&mut h);
            uid.hash(&mut h);
        }
        PeerIdentity::Windows { sid, .. } => {
            1u8.hash(&mut h);
            sid.hash(&mut h);
        }
        PeerIdentity::Unknown { .. } => {
            2u8.hash(&mut h);
        }
    }
    h.finish()
}

pub(in crate::ipc::server) fn handle_command_status(
    state: &Arc<DaemonState>,
    params: &CommandStatusParams,
) -> Result<IpcResponse, IpcError> {
    match state.command.status(params.job_id) {
        Ok(resp) => Ok(IpcResponse::CommandStatus(resp)),
        // The combed runtime declined: either it does not own this lane's job
        // (spec 004 FR-004) or the in-memory job is gone entirely.
        Err(crate::command::CommandError::UnknownJob(job_id)) => {
            // 1. Ask the lane that owns it. A PTY or watch job answers here
            //    with REAL counters. Constitution VII forbids discarding a live
            //    job behind a bare error, and `command_status` is today the only
            //    surface exposing a finished PTY job's exit code at all.
            #[cfg(any(unix, windows))]
            if let Some(resp) = state.pty.status(job_id) {
                return Ok(IpcResponse::CommandStatus(resp));
            }
            if let Some(resp) = state.watch.status(job_id) {
                return Ok(IpcResponse::CommandStatus(resp));
            }
            // 2. No live owner. Consult the persisted receipt: a known terminal
            //    outcome from disk beats a bare "unknown job" (constitution VII
            //    honest degradation).
            restart_marked_status_from_receipt(state, job_id).map_or_else(
                || {
                    Err(map_command_error(crate::command::CommandError::UnknownJob(
                        job_id,
                    )))
                },
                |resp| Ok(IpcResponse::CommandStatus(resp)),
            )
        }
        Err(e) => Err(map_command_error(e)),
    }
}

/// TC-B3 / spec 004: reconstruct a terminal [`CommandStatusResponse`] from a
/// persisted job receipt, or `None` if no receipt exists (then the caller
/// decides between `JobLost` and `UnknownJob`). Stamps the receipt's restart
/// marker as a side effect so the persisted row records that it was read
/// post-restart.
///
/// Counters come from the evidence captured at the terminal transition, so a
/// reconstructed pass is USABLE rather than merely labelled -- that gap is what
/// caused a reporting session to discard two passing suites and re-run them.
/// Receipts written before the evidence migration carry none, and that absence
/// is reported as-is rather than being disguised.
fn restart_marked_status_from_receipt(
    state: &Arc<DaemonState>,
    job_id: terminal_commander_core::JobId,
) -> Option<crate::ipc::protocol::CommandStatusResponse> {
    use terminal_commander_core::{BucketId, JobState, ProbeId};

    use crate::ipc::protocol::OutcomeTrust;

    let wire = job_id.to_wire_string();
    let row = state.store.get_job_receipt(&wire).ok().flatten()?;

    let state_enum = match row.terminal_state.as_str() {
        "exited" => JobState::Exited,
        "cancelled" => JobState::Cancelled,
        // Any other persisted label (incl. "failed" and the conservative
        // fallback) maps to Failed: a terminal, non-success state.
        _ => JobState::Failed,
    };
    // Recover the bucket id from the receipt; a parse failure (corrupt row)
    // falls back to a fresh id rather than failing the whole status read.
    let bucket_id = BucketId::parse_wire(&row.bucket_id).unwrap_or_default();
    // The events_emitted count was persisted in the small JSON object; a
    // parse miss is non-fatal (counts default to 0).
    let events_emitted = serde_json::from_str::<serde_json::Value>(&row.final_signal_counts)
        .ok()
        .and_then(|v| v.get("events_emitted").and_then(serde_json::Value::as_u64))
        .unwrap_or(0);

    // spec 004 FR-002: the evidence a live observer would have had. `None` for
    // pre-migration rows; those report zero counters, which the agent-facing
    // contract explicitly covers.
    let evidence = row
        .metrics_json
        .as_deref()
        .and_then(|m| serde_json::from_str::<serde_json::Value>(m).ok());
    let num = |key: &str| -> u64 {
        evidence
            .as_ref()
            .and_then(|v| v.get(key).and_then(serde_json::Value::as_u64))
            .unwrap_or(0)
    };
    let duration_ms = evidence
        .as_ref()
        .and_then(|v| v.get("duration_ms").and_then(serde_json::Value::as_u64));
    let probe_id = evidence
        .as_ref()
        .and_then(|v| v.get("probe_id").and_then(serde_json::Value::as_str))
        .and_then(|s| ProbeId::parse_wire(s).ok())
        .unwrap_or_default();

    // spec 004 D1: abandonment rides the trust indicator. The terminal state
    // stays the truthful `Cancelled` and the exit code stays absent -- an
    // abandoned job did not fail, and reporting it as a failure would be the
    // exact false negative this feature exists to remove.
    let abandoned = row.end_cause.as_deref() == Some("abandoned");
    let (state_enum, exit_code, outcome_trust) = if abandoned {
        (JobState::Cancelled, None, OutcomeTrust::Abandoned)
    } else {
        (state_enum, row.exit_code, OutcomeTrust::Reconstructed)
    };

    // Best-effort: stamp the restart marker on the durable row. A failure
    // here does not change the response we return.
    let _ = state.store.mark_job_receipt_restarted(&wire);

    Some(crate::ipc::protocol::CommandStatusResponse {
        job_id,
        bucket_id,
        probe_id,
        state: state_enum,
        frames_total: num("frames_total"),
        frames_stdout: num("frames_stdout"),
        frames_stderr: num("frames_stderr"),
        bytes_total: num("bytes_total"),
        events_emitted,
        frames_suppressed: num("frames_suppressed"),
        frames_suppressed_progress: num("frames_suppressed_progress"),
        frames_suppressed_dedupe: num("frames_suppressed_dedupe"),
        exit_code,
        signal: None,
        duration_ms,
        // The no-silence frame tail is memory-only by design (spec D2:
        // constitution III bars raw frames from persistent output). Its absence
        // here does NOT mean the command produced no output.
        receipt: None,
        // Derived, never set independently: true whenever the outcome was not
        // observed live.
        restarted: outcome_trust != OutcomeTrust::Observed,
        outcome_trust,
    })
}

pub(in crate::ipc::server) fn handle_command_output_tail(
    state: &Arc<DaemonState>,
    params: &CommandOutputTailParams,
) -> Result<IpcResponse, IpcError> {
    let rec = state.jobs.get(params.job_id).ok_or_else(|| {
        IpcError::new(
            IpcErrorCode::UnknownJob,
            format!("unknown job: {}", params.job_id),
        )
    })?;
    let probe_id = rec.config.probe_id;
    let max_lines = (params.max_lines as usize).min(MAX_TAIL_LINES);
    let max_bytes = (params.max_bytes as usize).min(MAX_TAIL_BYTES);
    // NotFound = ring absent (job had no ring yet); treat as empty tail
    let mut tail = match state.rings.tail_frames(probe_id, max_lines, max_bytes) {
        Ok(t) => t,
        Err(terminal_commander_core::ContextError::NotFound(_)) => {
            terminal_commander_core::RingTail {
                lines: vec![],
                evicted_frames: 0,
                truncated: false,
            }
        }
        Err(e) => return Err(IpcError::new(IpcErrorCode::Internal, e.to_string())),
    };
    if params.strip_ansi {
        tail.lines = tail
            .lines
            .into_iter()
            .map(|line| terminal_commander_probes::strip_ansi(&line))
            .collect();
    }
    let frame_count = state.rings.frame_count(probe_id);
    // Safe: tail.lines.len() is bounded by MAX_TAIL_LINES (200), fits u32.
    #[allow(clippy::cast_possible_truncation)]
    let returned_lines = tail.lines.len() as u32;
    let truncated_lines = frame_count > tail.lines.len();
    let truncated_bytes = tail.truncated;
    Ok(IpcResponse::CommandOutputTail(CommandOutputTailResponse {
        job_id: params.job_id,
        lines: tail.lines,
        returned_lines,
        truncated_lines,
        truncated_bytes,
        evicted_frames: tail.evicted_frames,
    }))
}
