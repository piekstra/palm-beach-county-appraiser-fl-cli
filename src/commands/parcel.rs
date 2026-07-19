//! `parcel` — the full record for one parcel control number.

use crate::commands::{parse_pcn, Ctx};
use crate::error::AppError;
use crate::formatter;
use crate::model::Parcel;

pub fn run(ctx: &Ctx, raw_pcn: &str, details: bool) -> Result<(), AppError> {
    let pcn = parse_pcn(raw_pcn)?;
    ctx.log(&format!("looking up parcel {}", pcn.bare()));

    let attrs = ctx
        .api
        .parcel(pcn.bare())?
        .ok_or_else(|| AppError::NotFound(format!("no parcel with control number {pcn}")))?;

    let mut parcel = Parcel::from_attrs(&attrs).ok_or_else(|| {
        AppError::Upstream("the parcel layer returned a row without a parcel id".into())
    })?;

    // Building structure lives on the county's web page, not the ArcGIS layer,
    // so it costs a second request and is opt-in.
    if details {
        parcel.building = ctx.details(&pcn)?.building;
    }

    formatter::print_parcel(&parcel, ctx.json);
    Ok(())
}
