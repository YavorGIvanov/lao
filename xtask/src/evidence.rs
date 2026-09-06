use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fs::File, io::Read, path::Path};

const LIMIT: u64 = 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Report {
    schema: u8,
    evidence: Evidence,
    status: Status,
    host: String,
    fixture_sha256: String,
    verifier_sha256: String,
    boundary: Boundary,
    cache: Cache,
    baseline: Config,
    candidate: Config,
    tasks: Vec<String>,
    rounds: u16,
    trials: Vec<Trial>,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Evidence {
    Synthetic,
    Measured,
}

#[derive(Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Status {
    Complete,
    Incomplete,
    Refused,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Boundary {
    DispatchThroughVerification,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum Cache {
    Cold,
    Warm,
}

#[derive(Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Config {
    source_revision: String,
    artifacts_sha256: String,
    runtime: String,
    model: String,
    harness: String,
    context: u32,
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Arm {
    Baseline,
    Candidate,
}

#[derive(Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Worker {
    Complete,
    Failed,
    Timeout,
    InfrastructureInvalid,
}

#[derive(Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
enum Route {
    Cloud,
    Local,
    LocalThenCloud,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Trial {
    task: String,
    round: u16,
    arm: Arm,
    observed: Config,
    route: Route,
    worker: Worker,
    verified: bool,
    scope_ok: bool,
    elapsed_ms: u64,
}

#[derive(Serialize)]
struct Summary {
    evidence: Evidence,
    claim: &'static str,
    cache: Cache,
    tasks: usize,
    rounds: u16,
    baseline: Counts,
    candidate: Counts,
    paired_success_delta_pp: f64,
}

#[derive(Serialize)]
struct Counts {
    trials: usize,
    verified: usize,
    worker_complete: usize,
    worker_failed: usize,
    timeouts: usize,
    scope_failures: usize,
    local_only: usize,
    local_then_cloud: usize,
    median_elapsed_ms: f64,
}

pub(super) fn run(path: &Path) -> super::Result<()> {
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|file| file.take(LIMIT + 1).read_to_end(&mut bytes))
        .map_err(|_| "cannot read evidence file")?;
    let summary = summarize(&bytes)?;
    println!("{}", serde_json::to_string_pretty(&summary)?);
    Ok(())
}

fn summarize(bytes: &[u8]) -> Result<Summary, &'static str> {
    if bytes.len() as u64 > LIMIT {
        return Err("evidence exceeds 1 MiB");
    }
    let report: Report = serde_json::from_slice(bytes).map_err(|_| "invalid evidence schema")?;
    if report.schema != 1 || report.status != Status::Complete {
        return Err("evidence must use schema 1 and be complete");
    }
    let Boundary::DispatchThroughVerification = report.boundary;
    if !identity(&report.host)
        || !hash(&report.fixture_sha256, 64)
        || !hash(&report.verifier_sha256, 64)
        || !config(&report.baseline)
        || !config(&report.candidate)
    {
        return Err("invalid evidence identity");
    }
    let tasks: BTreeSet<_> = report.tasks.iter().collect();
    if tasks.is_empty()
        || tasks.len() > 100
        || tasks.len() != report.tasks.len()
        || tasks.iter().any(|task| !identity(task))
        || !(1..=5).contains(&report.rounds)
    {
        return Err("invalid pilot task or round plan");
    }
    let mut seen = BTreeSet::new();
    for trial in &report.trials {
        let expected = match trial.arm {
            Arm::Baseline => &report.baseline,
            Arm::Candidate => &report.candidate,
        };
        if !tasks.contains(&trial.task)
            || !(1..=report.rounds).contains(&trial.round)
            || !seen.insert((&trial.task, trial.round, trial.arm))
        {
            return Err("unexpected or duplicate trial");
        }
        if trial.observed != *expected {
            return Err("observed configuration differs from declared arm");
        }
        if trial.worker == Worker::InfrastructureInvalid {
            return Err("infrastructure-invalid pair cannot support a comparison");
        }
        if trial.elapsed_ms == 0 || trial.elapsed_ms > 86_400_000 {
            return Err("pilot timing must be positive and at most 24 hours");
        }
    }
    if seen.len() != tasks.len() * usize::from(report.rounds) * 2 {
        return Err("incomplete paired evidence");
    }
    let baseline = counts(&report.trials, Arm::Baseline);
    let candidate = counts(&report.trials, Arm::Candidate);
    let delta =
        100.0 * (candidate.verified as f64 - baseline.verified as f64) / baseline.trials as f64;
    Ok(Summary {
        evidence: report.evidence,
        claim: "exploratory counts only; no promotion, tail latency, or savings claim",
        cache: report.cache,
        tasks: tasks.len(),
        rounds: report.rounds,
        baseline,
        candidate,
        paired_success_delta_pp: delta,
    })
}

fn identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
}

fn hash(value: &str, len: usize) -> bool {
    value.len() == len && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn config(value: &Config) -> bool {
    hash(&value.source_revision, 40)
        && hash(&value.artifacts_sha256, 64)
        && identity(&value.runtime)
        && identity(&value.model)
        && identity(&value.harness)
        && (1..=262_144).contains(&value.context)
}

fn counts(trials: &[Trial], arm: Arm) -> Counts {
    let rows: Vec<_> = trials.iter().filter(|trial| trial.arm == arm).collect();
    let mut times: Vec<_> = rows.iter().map(|trial| trial.elapsed_ms).collect();
    times.sort_unstable();
    let n = rows.len();
    Counts {
        trials: n,
        verified: rows
            .iter()
            .filter(|row| row.verified && row.scope_ok)
            .count(),
        worker_complete: rows
            .iter()
            .filter(|row| row.worker == Worker::Complete)
            .count(),
        worker_failed: rows
            .iter()
            .filter(|row| row.worker == Worker::Failed)
            .count(),
        timeouts: rows
            .iter()
            .filter(|row| row.worker == Worker::Timeout)
            .count(),
        scope_failures: rows.iter().filter(|row| !row.scope_ok).count(),
        local_only: rows.iter().filter(|row| row.route == Route::Local).count(),
        local_then_cloud: rows
            .iter()
            .filter(|row| row.route == Route::LocalThenCloud)
            .count(),
        median_elapsed_ms: (times[(n - 1) / 2] as f64 + times[n / 2] as f64) / 2.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const EXAMPLE: &[u8] = include_bytes!("../../docs/benchmarks/example.json");

    #[test]
    fn paired_evidence_counts_verified_work_independently_of_worker_status() {
        let summary = summarize(EXAMPLE).unwrap();
        assert!(matches!(summary.evidence, Evidence::Synthetic));
        assert_eq!(summary.baseline.verified, 2);
        assert_eq!(summary.candidate.trials, 2);
        assert_eq!(summary.candidate.worker_complete, 1);
        assert_eq!(summary.candidate.worker_failed, 1);
        assert_eq!(summary.candidate.verified, 1);
        assert_eq!(summary.candidate.median_elapsed_ms, 1500.0);
        assert_eq!(summary.paired_success_delta_pp, -50.0);
    }

    #[test]
    fn incomplete_or_changed_evidence_cannot_publish_a_comparison() {
        let original: serde_json::Value = serde_json::from_slice(EXAMPLE).unwrap();
        let mut missing = original.clone();
        missing["trials"].as_array_mut().unwrap().pop();
        assert_eq!(
            summarize(&serde_json::to_vec(&missing).unwrap()).err(),
            Some("incomplete paired evidence")
        );
        let mut changed = original;
        changed["trials"][0]["observed"]["runtime"] = "different-version".into();
        assert_eq!(
            summarize(&serde_json::to_vec(&changed).unwrap()).err(),
            Some("observed configuration differs from declared arm")
        );
    }
}
