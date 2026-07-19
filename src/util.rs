//! Small shared helpers: epoch-millisecond dates, SQL-literal escaping, and
//! the address/owner normalization the ArcGIS `LIKE` queries need.

/// Convert days-since-epoch to a civil `(year, month, day)`.
/// Howard Hinnant's `civil_from_days` algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// ArcGIS `esriFieldTypeDate` values arrive as epoch **milliseconds**; render
/// them as ISO `YYYY-MM-DD` (SPEC v1 §1.4). Epoch 0 means "no date" in this
/// dataset rather than 1970-01-01, so it yields `None`.
pub fn iso_from_epoch_ms(ms: i64) -> Option<String> {
    if ms == 0 {
        return None;
    }
    let (y, m, d) = civil_from_days(ms.div_euclid(86_400_000));
    Some(format!("{y:04}-{m:02}-{d:02}"))
}

/// Escape a value for embedding in an ArcGIS `where` clause. The layer is
/// read-only and anonymous, but a stray apostrophe (`O'BRIEN`) would still
/// break the query, so single quotes are doubled per SQL convention.
pub fn sql_escape(value: &str) -> String {
    value.replace('\'', "''")
}

/// Normalize a user's search text for a `LIKE` comparison: the county stores
/// situs addresses and owner names uppercase, and stray whitespace is common
/// when pasting. Collapses internal runs of whitespace to single spaces.
pub fn normalize_query(raw: &str) -> String {
    raw.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_epoch_ms_to_iso() {
        assert_eq!(
            iso_from_epoch_ms(946_684_800_000).as_deref(),
            Some("2000-01-01")
        );
        // A sale-shaped timestamp, matching the synthetic fixture in `model`.
        assert_eq!(
            iso_from_epoch_ms(1_592_179_200_000).as_deref(),
            Some("2020-06-15")
        );
    }

    #[test]
    fn treats_epoch_zero_as_no_date() {
        assert_eq!(iso_from_epoch_ms(0), None);
    }

    #[test]
    fn handles_pre_epoch_dates() {
        // Sales from the 1960s predate the epoch and must not wrap around.
        assert_eq!(
            iso_from_epoch_ms(-86_400_000).as_deref(),
            Some("1969-12-31")
        );
    }

    #[test]
    fn escapes_apostrophes_for_sql() {
        assert_eq!(sql_escape("O'BRIEN"), "O''BRIEN");
        assert_eq!(sql_escape("SMITH"), "SMITH");
    }

    #[test]
    fn normalizes_case_and_whitespace() {
        assert_eq!(normalize_query("  100   example st "), "100 EXAMPLE ST");
        assert_eq!(normalize_query("smith j"), "SMITH J");
    }
}
