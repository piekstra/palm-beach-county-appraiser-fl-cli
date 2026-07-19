//! `info` — machine-readable capability discovery (`cli-info/v1`, SPEC v1 §1.6).

use pk_cli_core::info::{AuthInfo, CliInfo};
use pk_cli_core::output;

use crate::commands::Ctx;
use crate::error::AppError;
use crate::update::REPO;

pub fn run(ctx: &Ctx) -> Result<(), AppError> {
    let _ = ctx;
    let info = CliInfo::new(
        "pbca",
        env!("CARGO_PKG_VERSION"),
        &format!("https://github.com/{REPO}"),
        AuthInfo {
            required: false,
            method: "none".into(),
            login_hint: None,
        },
        &["search", "parcel", "sales", "taxes", "building", "api"],
    );
    // No domain profile is declared: `utility/v1` describes billing accounts,
    // and property records are not a utility. Adding a `property/v1` profile
    // would need a second CLI in the domain plus a consumer paying for the
    // variance today (cli-common PROFILES.md).
    output::json(&serde_json::to_value(&info).unwrap_or_default());
    Ok(())
}
