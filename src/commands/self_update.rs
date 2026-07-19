//! `self-update` — update pbca to the latest GitHub release, via the family
//! updater (`pk-cli-selfupdate`).

use pk_cli_selfupdate::SelfUpdateArgs;

use crate::commands::Ctx;
use crate::error::AppError;
use crate::update;

pub fn run(ctx: &Ctx, args: &SelfUpdateArgs) -> Result<(), AppError> {
    ctx.log(&format!("checking github releases for {}…", update::REPO));
    update::updater().run(args, ctx.json, ctx.quiet)
}
