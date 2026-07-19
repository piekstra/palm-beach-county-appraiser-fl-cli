//! Parcel Control Number ("PCN") parsing.
//!
//! A PCN is 17 digits. The county displays it grouped —
//! `12-34-56-78-90-123-4567` (2-2-2-2-2-3-4) — but the ArcGIS `PARID` column
//! stores it undashed, so every query strips the separators. Both forms parse.

use pk_cli_core::CliError;

/// Digit widths of the dashed display form, in order.
const GROUPS: [usize; 7] = [2, 2, 2, 2, 2, 3, 4];

/// Total digits in a PCN.
pub const LEN: usize = 17;

/// A validated 17-digit parcel control number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pcn(String);

impl Pcn {
    /// Parse a PCN in either the dashed display form or bare digits. Spaces,
    /// dashes, and dots are all accepted as separators since people paste from
    /// the county site, a tax bill, and a spreadsheet interchangeably.
    pub fn parse(raw: &str) -> Result<Self, CliError> {
        let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
        let junk = raw
            .chars()
            .any(|c| !c.is_ascii_digit() && !matches!(c, '-' | ' ' | '.' | '_'));
        if junk {
            return Err(CliError::Usage(format!(
                "`{raw}` is not a parcel control number — expected {LEN} digits, \
                 e.g. 12-34-56-78-90-123-4567 or 12345678901234567"
            )));
        }
        if digits.len() != LEN {
            return Err(CliError::Usage(format!(
                "a parcel control number has {LEN} digits, `{raw}` has {} — \
                 e.g. 12-34-56-78-90-123-4567",
                digits.len()
            )));
        }
        Ok(Pcn(digits))
    }

    /// The bare 17-digit form, as stored in `PARID`. This is what queries use.
    pub fn bare(&self) -> &str {
        &self.0
    }

    /// The dashed display form the county prints, `12-34-56-78-90-123-4567`.
    pub fn dashed(&self) -> String {
        let mut out = String::with_capacity(LEN + GROUPS.len() - 1);
        let mut rest = self.0.as_str();
        for (i, width) in GROUPS.iter().enumerate() {
            if i > 0 {
                out.push('-');
            }
            let (head, tail) = rest.split_at(*width);
            out.push_str(head);
            rest = tail;
        }
        out
    }
}

impl std::fmt::Display for Pcn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.dashed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Synthetic PCNs only — never a real parcel (see AGENTS.md).
    const BARE: &str = "12345678901234567";
    const DASHED: &str = "12-34-56-78-90-123-4567";

    #[test]
    fn parses_bare_and_dashed_to_the_same_value() {
        assert_eq!(Pcn::parse(BARE).unwrap(), Pcn::parse(DASHED).unwrap());
    }

    #[test]
    fn round_trips_between_forms() {
        let p = Pcn::parse(BARE).unwrap();
        assert_eq!(p.bare(), BARE);
        assert_eq!(p.dashed(), DASHED);
        assert_eq!(Pcn::parse(&p.dashed()).unwrap().bare(), BARE);
    }

    #[test]
    fn display_uses_the_dashed_form() {
        assert_eq!(Pcn::parse(BARE).unwrap().to_string(), DASHED);
    }

    #[test]
    fn tolerates_spaces_dots_and_underscores() {
        assert!(Pcn::parse("12 34 56 78 90 123 4567").is_ok());
        assert!(Pcn::parse("12.34.56.78.90.123.4567").is_ok());
    }

    #[test]
    fn rejects_wrong_length() {
        let err = Pcn::parse("1234").unwrap_err();
        assert_eq!(err.exit_code(), 2, "bad input is a usage error");
        assert!(Pcn::parse(&"9".repeat(18)).is_err());
        assert!(Pcn::parse("").is_err());
    }

    #[test]
    fn rejects_non_numeric_input() {
        // An address must not be mistaken for a PCN — it should say so, rather
        // than silently stripping letters down to a few stray digits.
        assert!(Pcn::parse("100 EXAMPLE ST").is_err());
        assert!(Pcn::parse("SMITH JOHN").is_err());
    }
}
