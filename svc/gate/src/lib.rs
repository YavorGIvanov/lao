#[allow(dead_code)]
mod net;
#[allow(dead_code)]
mod policy;

use lao_gate_api::Status;
use lao_route_api::Policy;
use lao_run_api::Endpoint;
use std::{error::Error, net::TcpListener};

#[derive(Clone, Copy)]
pub enum CodexCloud {
    Api,
    ChatGpt,
}

#[derive(Clone, Copy)]
pub enum ClaudeCloud {
    Bearer,
    Key,
}

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

pub fn canary(
    listener: TcpListener,
    policy: impl Policy + 'static,
    endpoint: Endpoint,
    codex: [u8; 32],
    claude: [u8; 32],
) -> Result<(), Box<dyn Error + Send + Sync>> {
    tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()?
        .block_on(net::canary(listener, policy, endpoint, codex, claude))
}

pub fn installed(
    listener: TcpListener,
    policy: impl Policy + 'static,
    endpoint: Endpoint,
    codex: [u8; 32],
    claude: [u8; 32],
    codex_cloud: CodexCloud,
    claude_cloud: ClaudeCloud,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()?
        .block_on(net::installed(
            listener,
            policy,
            endpoint,
            codex,
            claude,
            codex_cloud,
            claude_cloud,
        ))
}

pub fn status() -> Status {
    Status::Stub
}
