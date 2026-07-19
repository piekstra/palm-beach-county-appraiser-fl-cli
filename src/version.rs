//! Version and build information.

/// Crate version, from Cargo at build time.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// One-line build banner: version, target OS, and architecture. Shown by
/// `pbca --version` (via clap) and available for diagnostics.
pub fn build_info() -> String {
    format!(
        "pbca {} ({} {})",
        VERSION,
        std::env::consts::OS,
        std::env::consts::ARCH,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_semver_like() {
        let parts: Vec<&str> = VERSION.split('.').collect();
        assert!(parts.len() >= 2, "expected semver-like version: {VERSION}");
        assert!(parts[0].parse::<u32>().is_ok());
    }

    #[test]
    fn build_info_names_the_binary_and_arch() {
        let info = build_info();
        assert!(info.contains("pbca"));
        assert!(info.contains(std::env::consts::ARCH));
    }
}
