#[allow(dead_code)]
mod net;
#[allow(dead_code)]
mod policy;

use lao_gate_api::Status;

pub fn status() -> Status {
    Status::Stub
}
