//! REST API layer for SolTrace — Axum-based query endpoints.

pub mod auth;
pub mod handlers;
pub mod routes;

pub use auth::JwtConfig;
pub use routes::build_router;
