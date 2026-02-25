pub mod acquisition;
pub mod analytics_impl;
pub mod auth;
pub mod backend;
pub mod queries;
pub mod schema;
pub mod session;
pub mod share;
pub mod website;

pub use backend::DuckDbBackend;

/// Re-export the `duckdb` crate so consumers (especially tests) can use
/// `sparklytics_duckdb::duckdb::params!` without an extra dependency.
pub use duckdb;
