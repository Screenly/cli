use crate::authentication::Authentication;

pub mod edge_app;
pub mod version;
pub mod asset;

pub struct Api {
    pub authentication: Authentication
}
