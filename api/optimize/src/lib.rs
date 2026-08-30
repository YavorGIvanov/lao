use std::io;

pub type Probe = Box<dyn FnOnce() -> io::Result<()> + Send + 'static>;

pub struct Plan {
    claude: Probe,
    codex: Probe,
}

impl Plan {
    pub fn new(
        claude: impl FnOnce() -> io::Result<()> + Send + 'static,
        codex: impl FnOnce() -> io::Result<()> + Send + 'static,
    ) -> Self {
        Self {
            claude: Box::new(claude),
            codex: Box::new(codex),
        }
    }

    pub fn into_probes(self) -> (Probe, Probe) {
        (self.claude, self.codex)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum State {
    Idle,
    Warming,
    Ready,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Start {
    Started,
    Busy,
}

pub trait Optimize: Send + Sync {
    fn start(&self, plan: Plan) -> io::Result<Start>;
    fn state(&self) -> State;
}
