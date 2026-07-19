//! `palm_beach_county_appraiser_fl` — the library behind the `pbca` CLI.
//!
//! Business logic (the ArcGIS client, the parcel models, the summary-page
//! scraper, output formatting) lives here so it is unit-testable and reusable;
//! `main.rs` is a thin binary that parses arguments and dispatches into
//! [`commands`].

pub mod cli;
pub mod client;
pub mod commands;
pub mod config;
pub mod details;
pub mod error;
pub mod formatter;
pub mod model;
pub mod pcn;
pub mod update;
pub mod util;
pub mod version;
