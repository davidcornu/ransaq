#![warn(missing_docs)]
#![warn(clippy::missing_docs_in_private_items)]

//! A crawler for the [SAQ](https://www.saq.com/en/)'s product catalog.
//!
//! It was written primarily as an interesting challenge and an opportunity
//! to learn several Rust libraries and features.
//!
//! ## Setup
//!
//! - Run `cargo doc --open` to view the docs
//! - See the [`db`] module docs for database setup

mod crawler;
mod db;
mod saq;

use color_eyre::eyre::Result;
use tracing::warn;
use tracing_subscriber::EnvFilter;

/// Global setup for the application
/// - Loads additional environment variables from `.env` (using [`dotenv`](dotenv))
/// - Initializes [`color_eyre`](color_eyre)
/// - Initializes [`tracing_subscriber`](tracing_subscriber)
fn setup() -> Result<()> {
    if let Err(e) = dotenv::dotenv() {
        warn!("failed to load .env file: {}", e);
    }

    if std::env::var("RUST_LIB_BACKTRACE").is_err() {
        std::env::set_var("RUST_LIB_BACKTRACE", "full")
    }

    color_eyre::install()?;

    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "ransaq=trace,info")
    }

    tracing_subscriber::fmt::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    Ok(())
}

/// Kicks off a full crawl
#[tokio::main]
async fn main() -> Result<()> {
    setup()?;

    crawler::crawl().await?;

    Ok(())
}
