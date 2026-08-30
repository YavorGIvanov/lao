use std::{io, net::SocketAddr, sync::Arc};

pub use lao_core_api::{Fault, Status};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    Light,
    Auto,
    Maximum,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Budget {
    pub bytes: u64,
    pub threads: u16,
    pub target_context: u32,
}

pub struct Endpoint {
    addr: SocketAddr,
    bearer: Box<str>,
}

impl Endpoint {
    pub fn new(addr: SocketAddr, bearer: impl Into<Box<str>>) -> Self {
        Self {
            addr,
            bearer: bearer.into(),
        }
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn bearer(&self) -> &str {
        &self.bearer
    }
}

/// Blocking: the first call may verify a model and spawn a runtime process.
pub trait Local: Send + Sync {
    fn endpoint(&self) -> io::Result<Arc<Endpoint>>;
}
