use crate::authentication::Authentication;

pub mod asset;
pub mod edge_app;
pub mod version;

pub struct Api {
    pub authentication: Authentication,
}
