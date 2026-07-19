//! `auth status` — credential state.
//!
//! This CLI has no credentials: the appraiser's parcel data is anonymous public
//! records. The command exists anyway because SPEC v1 §1.2 requires it of every
//! family member, including the credential-free ones — drivers can then read
//! `required: false` uniformly instead of special-casing which CLIs have logins.

use pk_cli_auth::AuthStatus;

use crate::commands::Ctx;
use crate::error::AppError;

pub fn status(ctx: &Ctx) -> Result<(), AppError> {
    AuthStatus::not_required().emit(ctx.json);
    Ok(())
}
