#[allow(dead_code)]
mod net;
#[allow(dead_code)]
mod policy;

use lao_gate_api::Status;
use std::{error::Error, net::TcpListener};

pub fn closed(listener: TcpListener) -> Result<(), Box<dyn Error + Send + Sync>> {
    tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()?
        .block_on(net::closed(listener))
}

pub fn status() -> Status {
    Status::Stub
}
