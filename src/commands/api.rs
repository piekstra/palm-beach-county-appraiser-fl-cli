//! `api` — raw ArcGIS passthrough (SPEC v1 §1.2).
//!
//! The parcel layer publishes ~58 columns and a dozen query options this CLI
//! does not model. Rather than grow a flag for each, `api` hands the request
//! straight through and prints whatever comes back.

use pk_cli_http::ApiArgs;

use crate::commands::Ctx;
use crate::error::AppError;

pub fn run(ctx: &Ctx, args: &ApiArgs) -> Result<(), AppError> {
    let url = args.url(ctx.api.base());
    let method = args.parsed_method()?;
    ctx.log(&format!("{method} {url}"));

    let mut req = ctx.api.http().request(method, &url);
    if let Some(body) = args.parsed_body()? {
        req = req.json(&body);
    }
    let value = pk_cli_http::json_response(req.send()?)?;

    // The passthrough is a debugging and scripting tool, so the response is
    // printed verbatim — no DTO shaping, no schema tag.
    if ctx.json {
        pk_cli_core::output::json(&value);
    } else {
        pk_cli_core::output::render(&value);
    }
    Ok(())
}
