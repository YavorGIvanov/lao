use lao_route_api::{Context, Decision, Policy};

pub struct Router;

impl Policy for Router {
    fn decide(&self, context: Context) -> Decision {
        if context.is_canary() {
            Decision::Local
        } else {
            Decision::Cloud
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lao_route_api::{Client, Op};

    #[test]
    fn only_canary_is_local() {
        assert_eq!(
            Router.decide(Context::new(Client::Codex, Op::Responses)),
            Decision::Cloud
        );
        assert_eq!(
            Router.decide(Context::canary(Client::Claude, Op::Messages)),
            Decision::Local
        );
    }
}
