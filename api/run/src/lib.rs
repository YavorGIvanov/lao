use std::net::SocketAddr;

pub use lao_core_api::{Fault, Status};

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
