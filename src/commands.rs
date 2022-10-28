use crate::Authentication;

pub struct ScreenCommand {
    authentication: Authentication,
}

impl ScreenCommand {
    fn new(authentication: Authentication) -> Self {
        Self { authentication }
    }

    fn list() {}

    fn get(_id: &str) {}
}
