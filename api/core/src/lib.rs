use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Active,
    Stub,
    Unsupported,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Fault {
    pub code: String,
    pub retry: String,
    pub message: String,
}

impl Fault {
    pub fn unsupported() -> Self {
        Self {
            code: "unsupported".into(),
            retry: "never".into(),
            message: "component is not enabled".into(),
        }
    }
}
