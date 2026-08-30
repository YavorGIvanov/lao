#[allow(dead_code)]
mod net;
#[allow(dead_code)]
mod policy;

use lao_gate_api::Status;
use lao_route_api::Policy;
use std::{error::Error, io, net::TcpListener, sync::Arc};

use lao_run_api::{Endpoint, Local};

#[derive(Clone, Copy)]
pub enum CodexCloud {
    Api,
    ChatGpt,
}

pub fn closed(
    listener: TcpListener,
    policy: impl Policy + 'static,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    serve(listener, policy, None, [0; 64], [0; 64], CodexCloud::Api)
}

pub fn canary(
    listener: TcpListener,
    policy: impl Policy + 'static,
    endpoint: Endpoint,
    codex: [u8; 64],
    claude: [u8; 64],
) -> Result<(), Box<dyn Error + Send + Sync>> {
    serve(
        listener,
        policy,
        Some(Arc::new(Ready(Arc::new(endpoint)))),
        codex,
        claude,
        CodexCloud::Api,
    )
}

pub fn installed(
    listener: TcpListener,
    policy: impl Policy + 'static,
    local: Arc<dyn Local>,
    codex: [u8; 64],
    claude: [u8; 64],
    codex_cloud: CodexCloud,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    serve(listener, policy, Some(local), codex, claude, codex_cloud)
}

fn serve(
    listener: TcpListener,
    policy: impl Policy + 'static,
    local: Option<Arc<dyn Local>>,
    codex: [u8; 64],
    claude: [u8; 64],
    codex_cloud: CodexCloud,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()?
        .block_on(net::configured(
            listener,
            policy,
            local,
            codex,
            claude,
            codex_cloud,
        ))
}

pub fn status() -> Status {
    Status::Active
}

pub(crate) struct Ready(pub Arc<Endpoint>);

impl Local for Ready {
    fn endpoint(&self) -> io::Result<Arc<Endpoint>> {
        Ok(self.0.clone())
    }
}
