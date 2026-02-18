/// Re-export `Config` from `sparklytics-core` for use within this crate.
///
/// All environment-variable parsing lives in `sparklytics-core` so it can be
/// shared with integration tests and future crates without depending on the
/// full server.
pub use sparklytics_core::config::Config;
