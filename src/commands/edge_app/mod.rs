pub mod app;
pub mod instance;
pub mod instance_manifest;
pub mod manifest;
pub mod manifest_auth;
pub(crate) mod server;
pub(crate) mod setting;
pub mod test_utils;
pub mod utils;

use crate::api::Api;
use crate::authentication::Authentication;

pub struct EdgeAppCommand {
    api: Api,
}

impl EdgeAppCommand {
    pub fn new(authentication: Authentication) -> Self {
        Self {
            api: Api { authentication },
        }
    }
}
