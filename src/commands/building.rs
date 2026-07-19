//! `building` — structure details: year built, living area, square footage.

use crate::commands::{parse_pcn, Ctx};
use crate::error::AppError;
use crate::formatter;

pub fn run(ctx: &Ctx, raw_pcn: &str) -> Result<(), AppError> {
    let pcn = parse_pcn(raw_pcn)?;
    let building = ctx.details(&pcn)?.building;
    // A parcel with no building is vacant land, not an error.
    formatter::print_building(&pcn.dashed(), building.as_ref(), ctx.json);
    Ok(())
}
