#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Client {
    Codex,
    Claude,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Op {
    Responses,
    Compact,
    Models,
    Messages,
    Count,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Context {
    client: Client,
    op: Op,
    canary: bool,
}

impl Context {
    pub const fn new(client: Client, op: Op) -> Self {
        Self {
            client,
            op,
            canary: false,
        }
    }

    pub const fn canary(client: Client, op: Op) -> Self {
        Self {
            client,
            op,
            canary: true,
        }
    }

    pub const fn client(self) -> Client {
        self.client
    }

    pub const fn op(self) -> Op {
        self.op
    }

    pub const fn is_canary(self) -> bool {
        self.canary
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Decision {
    Local,
    Cloud,
}

pub trait Policy: Send + Sync {
    fn decide(&self, context: Context) -> Decision;
}
