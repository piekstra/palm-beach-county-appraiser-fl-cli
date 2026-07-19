//! Non-secret on-disk preferences, stored by the family config crate at
//! `${XDG_CONFIG_HOME:-~/.config}/pbca/config.json`.
//!
//! This tool has **no credentials** — the county's parcel data is anonymous
//! public records — so nothing here is ever a secret and the OS keychain is
//! never touched. These are just defaults that save typing.

use pk_cli_config::ConfigStore;
use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// Config directory / store name. Matches the binary name, per SPEC v1.
pub const APP: &str = "pbca";

/// The config keys `pbca config set` accepts, for validation and error text.
pub const KEYS: &[&str] = &["municipality", "limit"];

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Default municipality filter for searches, e.g. `JUPITER`. Narrows
    /// street-name searches that would otherwise match countywide.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub municipality: Option<String>,

    /// Default `--limit` for searches when the flag is not given.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

pub fn store() -> ConfigStore {
    ConfigStore::new(APP)
}

/// Load the saved preferences (a missing file is not an error).
pub fn load() -> Result<Config, AppError> {
    store().load()
}

/// Apply `key=value`, validating both. Returns the updated config to save.
pub fn set(cfg: &mut Config, key: &str, value: &str) -> Result<(), AppError> {
    match key {
        "municipality" => cfg.municipality = Some(crate::util::normalize_query(value)),
        "limit" => {
            let n: u32 = value.parse().map_err(|_| {
                AppError::Usage(format!("limit must be a whole number, got `{value}`"))
            })?;
            if n == 0 {
                return Err(AppError::Usage("limit must be at least 1".into()));
            }
            cfg.limit = Some(n);
        }
        other => return Err(unknown_key(other)),
    }
    Ok(())
}

/// Clear `key`. Returns `true` if it had been set.
pub fn unset(cfg: &mut Config, key: &str) -> Result<bool, AppError> {
    let had = match key {
        "municipality" => cfg.municipality.take().is_some(),
        "limit" => cfg.limit.take().is_some(),
        other => return Err(unknown_key(other)),
    };
    Ok(had)
}

fn unknown_key(key: &str) -> AppError {
    AppError::Usage(format!(
        "unknown config key `{key}` (known: {})",
        KEYS.join(", ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sets_and_unsets_known_keys() {
        let mut cfg = Config::default();
        set(&mut cfg, "municipality", "jupiter").unwrap();
        assert_eq!(cfg.municipality.as_deref(), Some("JUPITER"));
        assert!(unset(&mut cfg, "municipality").unwrap());
        // Unsetting an already-clear key is not an error, just `false`.
        assert!(!unset(&mut cfg, "municipality").unwrap());
    }

    #[test]
    fn normalizes_municipality_to_the_stored_casing() {
        let mut cfg = Config::default();
        set(&mut cfg, "municipality", "  west palm   beach ").unwrap();
        assert_eq!(cfg.municipality.as_deref(), Some("WEST PALM BEACH"));
    }

    #[test]
    fn rejects_bad_limits_and_unknown_keys() {
        let mut cfg = Config::default();
        assert_eq!(set(&mut cfg, "limit", "abc").unwrap_err().exit_code(), 2);
        assert!(set(&mut cfg, "limit", "0").is_err());
        assert!(set(&mut cfg, "nope", "x").is_err());
        assert!(unset(&mut cfg, "nope").is_err());
        set(&mut cfg, "limit", "25").unwrap();
        assert_eq!(cfg.limit, Some(25));
    }

    #[test]
    fn empty_config_serializes_to_an_empty_object() {
        // Keeps `config show` clean instead of emitting a wall of nulls.
        let json = serde_json::to_string(&Config::default()).unwrap();
        assert_eq!(json, "{}");
    }
}
