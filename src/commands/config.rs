//! `config` — non-secret preferences (SPEC v1 §1.2).
//!
//! Nothing stored here is sensitive: this tool has no credentials, so the OS
//! keychain is never involved.

use pk_cli_core::output;

use crate::cli::ConfigCmd;
use crate::commands::Ctx;
use crate::config;
use crate::error::AppError;

pub fn run(ctx: &Ctx, cmd: &ConfigCmd) -> Result<(), AppError> {
    let store = config::store();
    match cmd {
        ConfigCmd::Path => {
            println!("{}", store.path()?.display());
        }
        ConfigCmd::Show => {
            let cfg = config::load()?;
            let v = serde_json::to_value(&cfg).unwrap_or_default();
            if ctx.json {
                output::json(&v);
            } else if v.as_object().is_some_and(|o| o.is_empty()) {
                println!("no settings — defaults in use (see `pbca config set --help`)");
            } else {
                output::render(&v);
            }
        }
        ConfigCmd::Set { key, value } => {
            let mut cfg = config::load()?;
            config::set(&mut cfg, key, value)?;
            store.save(&cfg)?;
            if !ctx.quiet {
                eprintln!("set {key}");
            }
        }
        ConfigCmd::Unset { key } => {
            let mut cfg = config::load()?;
            let had = config::unset(&mut cfg, key)?;
            store.save(&cfg)?;
            if !ctx.quiet {
                eprintln!(
                    "{}",
                    if had {
                        format!("unset {key}")
                    } else {
                        format!("{key} was not set")
                    }
                );
            }
        }
    }
    Ok(())
}
