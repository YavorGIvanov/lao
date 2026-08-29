use lao_route_api::{Context, Decision, Policy};

pub struct Router;

impl Policy for Router {
    fn decide(&self, _: Context) -> Decision {
        Decision::Cloud
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lao_route_api::{Client, Op};

    #[test]
    fn defaults_to_cloud() {
        assert_eq!(
            Router.decide(Context::new(Client::Codex, Op::Responses)),
            Decision::Cloud
        );
    }
}
