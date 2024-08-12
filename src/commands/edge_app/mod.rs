pub mod app;
pub mod instance;
pub mod instance_manifest;
pub mod manifest;
pub mod manifest_auth;
pub(crate) mod server;
pub(crate) mod setting;
pub mod test_utils;
pub mod utils;

use crate::authentication::Authentication;

pub struct EdgeAppCommand {
    authentication: Authentication,
}

impl EdgeAppCommand {
    pub fn new(authentication: Authentication) -> Self {
        Self { authentication }
    }
}
