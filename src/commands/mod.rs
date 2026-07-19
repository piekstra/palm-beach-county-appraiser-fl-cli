//! Command implementations — one thin module per domain. Each function takes a
//! shared [`Ctx`] plus its parsed arguments, calls the client, and hands the
//! result to the [`formatter`](crate::formatter). `main.rs` only wires args to
//! these.

pub mod api;
pub mod auth;
pub mod building;
pub mod completions;
pub mod config;
pub mod info;
pub mod parcel;
pub mod sales;
pub mod search;
pub mod self_update;
pub mod taxes;

use crate::cli::Cli;
use crate::client::Arcgis;
use crate::config::Config;
use crate::details::{self, Details};
use crate::error::AppError;
use crate::pcn::Pcn;

/// Fallback for `--limit` when neither the flag nor the config sets one.
pub const DEFAULT_LIMIT: u32 = 25;

/// Per-invocation context: the ArcGIS client, the saved preferences, and the
/// global flags every command reads. Built once in `main`.
pub struct Ctx {
    pub api: Arcgis,
    pub cfg: Config,
    pub json: bool,
    pub verbose: bool,
    pub quiet: bool,
}

impl Ctx {
    pub fn new(cli: &Cli) -> Result<Self, AppError> {
        Ok(Ctx {
            api: Arcgis::new()?,
            cfg: crate::config::load()?,
            json: cli.common.json,
            verbose: cli.common.verbose,
            quiet: cli.common.quiet,
        })
    }

    /// Emit a diagnostic line (stderr) when `--verbose` and not `--quiet`.
    pub fn log(&self, msg: &str) {
        if self.verbose && !self.quiet {
            eprintln!("{msg}");
        }
    }

    /// Resolve `--limit`: the flag, then the `limit` config key, then the
    /// built-in default.
    pub fn limit(&self, flag: Option<u32>) -> u32 {
        flag.or(self.cfg.limit).unwrap_or(DEFAULT_LIMIT)
    }

    /// Fetch and parse the county's detail page for `pcn`.
    ///
    /// Separate from the ArcGIS layer on purpose: this one is a scrape of a
    /// public web page, so it is only reached by commands that need what the
    /// layer cannot provide (sale history, tax rolls, building structure).
    pub fn details(&self, pcn: &Pcn) -> Result<Details, AppError> {
        self.log(&format!("fetching the county detail page for {pcn}"));
        details::fetch(self.api.http(), pcn)
    }
}

/// Parse a PCN argument, mapping a bad value onto the usage exit code.
pub fn parse_pcn(raw: &str) -> Result<Pcn, AppError> {
    Pcn::parse(raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mirrors `Ctx::limit` without needing a live client to construct a `Ctx`.
    fn resolve(flag: Option<u32>, cfg: &Config) -> u32 {
        flag.or(cfg.limit).unwrap_or(DEFAULT_LIMIT)
    }

    #[test]
    fn limit_precedence_is_flag_then_config_then_default() {
        let mut cfg = Config::default();
        // No flag, no config → the built-in default.
        assert_eq!(resolve(None, &cfg), DEFAULT_LIMIT);
        // Config set, no flag → config wins.
        cfg.limit = Some(10);
        assert_eq!(resolve(None, &cfg), 10);
        // Flag always wins.
        assert_eq!(resolve(Some(5), &cfg), 5);
    }
}
