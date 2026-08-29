#[allow(dead_code)]
mod net;
#[allow(dead_code)]
mod policy;

use lao_gate_api::Status;
use lao_route_api::Policy;
use std::{error::Error, net::TcpListener};

pub fn closed(
    listener: TcpListener,
    policy: impl Policy + 'static,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()?
        .block_on(net::closed(listener, policy))
}

pub fn status() -> Status {
    Status::Stub
}
