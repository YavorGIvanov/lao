use std::{io, path::PathBuf, time::Duration};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Task {
    pub root: PathBuf,
    pub instruction: String,
    /// Exact repository-relative files the turn may change.
    pub allowed: Vec<PathBuf>,
    pub deadline: Duration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Outcome {
    Complete,
    AgentFailed,
    TimedOut,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Report {
    pub outcome: Outcome,
    pub changed: Vec<PathBuf>,
}

pub trait Agent: Send + Sync {
    /// Blocking: runs one bounded agent turn.
    fn turn(&self, task: &Task) -> io::Result<Report>;
}
