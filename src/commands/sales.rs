//! `sales` — every recorded sale for a parcel, newest first.

use crate::commands::{parse_pcn, Ctx};
use crate::error::AppError;
use crate::formatter;

pub fn run(ctx: &Ctx, raw_pcn: &str, limit: Option<u32>) -> Result<(), AppError> {
    let pcn = parse_pcn(raw_pcn)?;
    let mut sales = ctx.details(&pcn)?.sales;

    // Unlike `search`, an explicit `--limit` is the only cap here: a sale
    // history is short and truncating it by default would hide transfers.
    if let Some(n) = limit {
        sales.truncate(n as usize);
    }

    formatter::print_sales(&pcn.dashed(), &sales, ctx.json);
    Ok(())
}
