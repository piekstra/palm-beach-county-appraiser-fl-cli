//! `pbca self-update` — self-update from GitHub Releases via the family
//! updater (`pk-cli-selfupdate`). Release assets are named
//! `pbca-<target-triple>.tar.gz`; the triple is baked in by `build.rs`.

pub use pk_cli_selfupdate::{SelfUpdateArgs, Updater};

/// The GitHub repository releases are pulled from.
pub const REPO: &str = "piekstra/palm-beach-county-appraiser-fl-cli";

pub fn updater() -> Updater {
    Updater {
        repo: REPO.into(),
        // The name inside the release archive, which on Windows is `pbca.exe`.
        binary: format!("pbca{}", std::env::consts::EXE_SUFFIX),
        target: env!("BUILD_TARGET").into(),
        current: env!("CARGO_PKG_VERSION").into(),
    }
}
