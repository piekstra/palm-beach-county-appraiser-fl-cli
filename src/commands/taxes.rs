//! `taxes` — year-by-year values and levies for a parcel.

use crate::commands::{parse_pcn, Ctx};
use crate::error::AppError;
use crate::formatter;

pub fn run(ctx: &Ctx, raw_pcn: &str, limit: Option<u32>) -> Result<(), AppError> {
    let pcn = parse_pcn(raw_pcn)?;
    let details = ctx.details(&pcn)?;
    let mut years = details.tax_years;

    // Already newest-first, so a limit keeps the most recent years.
    if let Some(n) = limit {
        years.truncate(n as usize);
    }

    formatter::print_taxes(&pcn.dashed(), &years, &details.exemptions, ctx.json);
    Ok(())
}
