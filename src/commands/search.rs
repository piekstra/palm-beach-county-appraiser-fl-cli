//! `search` — find parcels by situs address, owner name, or PCN.

use crate::client::{PARCEL_LAYER, SEARCH_FIELDS};
use crate::commands::Ctx;
use crate::error::AppError;
use crate::formatter;
use crate::model::{ParcelList, ParcelSummary};
use crate::pcn::Pcn;
use crate::util::{normalize_query, sql_escape};

pub fn run(
    ctx: &Ctx,
    query: &str,
    owner: bool,
    municipality: Option<&str>,
    limit: Option<u32>,
) -> Result<(), AppError> {
    let normalized = normalize_query(query);
    if normalized.is_empty() {
        return Err(AppError::Usage("search needs something to match on".into()));
    }
    let limit = ctx.limit(limit);
    let municipality = municipality
        .map(normalize_query)
        .or_else(|| ctx.cfg.municipality.clone());

    let (where_clause, order_by) = build_query(&normalized, owner, municipality.as_deref());
    ctx.log(&format!("where {where_clause}"));

    // The exact server-side total, so the caller is told what was withheld
    // rather than left to infer it from a full page.
    let total = ctx.api.count(PARCEL_LAYER, &where_clause)?;
    let items: Vec<ParcelSummary> = if total == 0 {
        Vec::new()
    } else {
        ctx.api
            .query_all(PARCEL_LAYER, &where_clause, SEARCH_FIELDS, order_by, limit)?
            .iter()
            .filter_map(ParcelSummary::from_attrs)
            .collect()
    };

    formatter::print_search(&ParcelList::new(items, total), ctx.json);
    Ok(())
}

/// Build the ArcGIS `where` clause and sort order for a search.
///
/// Returned separately from the request so it can be unit-tested — a
/// malformed clause is the single most likely way this command breaks.
fn build_query(
    normalized: &str,
    owner: bool,
    municipality: Option<&str>,
) -> (String, &'static str) {
    // A bare PCN is unambiguous, so match it exactly however it was typed.
    // This makes `pbca search <pcn>` work without a mode flag.
    if !owner {
        if let Ok(pcn) = Pcn::parse(normalized) {
            return (format!("PARID='{}'", sql_escape(pcn.bare())), "PARID");
        }
    }

    let escaped = sql_escape(normalized);
    let (mut clause, order) = if owner {
        // Owner names are surname-first; both owner columns are searched.
        (
            format!("(OWNER_NAME1 LIKE '{escaped}%' OR OWNER_NAME2 LIKE '{escaped}%')"),
            "OWNER_NAME1,PARID",
        )
    } else {
        (
            format!("SITE_ADDR_STR LIKE '{escaped}%'"),
            "SITE_ADDR_STR,PARID",
        )
    };

    if let Some(m) = municipality.filter(|m| !m.is_empty()) {
        clause = format!("{clause} AND MUNICIPALITY='{}'", sql_escape(m));
    }
    (clause, order)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address_search_is_a_prefix_like() {
        let (w, order) = build_query("100 EXAMPLE", false, None);
        assert_eq!(w, "SITE_ADDR_STR LIKE '100 EXAMPLE%'");
        assert_eq!(order, "SITE_ADDR_STR,PARID");
    }

    #[test]
    fn owner_search_covers_both_owner_columns() {
        let (w, order) = build_query("SMITH J", true, None);
        assert_eq!(
            w,
            "(OWNER_NAME1 LIKE 'SMITH J%' OR OWNER_NAME2 LIKE 'SMITH J%')"
        );
        assert_eq!(order, "OWNER_NAME1,PARID");
    }

    #[test]
    fn a_bare_pcn_becomes_an_exact_match() {
        let (w, _) = build_query("12-34-56-78-90-123-4567", false, None);
        assert_eq!(w, "PARID='12345678901234567'");
        // ...and the undashed form resolves identically.
        let (w2, _) = build_query("12345678901234567", false, None);
        assert_eq!(w, w2);
    }

    #[test]
    fn owner_mode_never_reinterprets_the_query_as_a_pcn() {
        let (w, _) = build_query("12345678901234567", true, None);
        assert!(w.starts_with("(OWNER_NAME1 LIKE"));
    }

    #[test]
    fn municipality_narrows_either_mode() {
        let (w, _) = build_query("MAIN ST", false, Some("JUPITER"));
        assert_eq!(
            w,
            "SITE_ADDR_STR LIKE 'MAIN ST%' AND MUNICIPALITY='JUPITER'"
        );
        let (w, _) = build_query("DOE", true, Some("JUPITER"));
        assert!(w.ends_with(" AND MUNICIPALITY='JUPITER'"));
    }

    #[test]
    fn an_empty_municipality_adds_no_clause() {
        let (w, _) = build_query("MAIN ST", false, Some(""));
        assert!(!w.contains("MUNICIPALITY"));
    }

    #[test]
    fn apostrophes_are_escaped_not_injected() {
        // O'BRIEN must not terminate the SQL string literal.
        let (w, _) = build_query("O'BRIEN", true, None);
        assert!(w.contains("O''BRIEN"), "{w}");
        assert!(!w.contains("O'BRIEN%'"), "unescaped quote leaked: {w}");
    }

    #[test]
    fn injection_attempt_stays_inside_the_literal() {
        let (w, _) = build_query("X' OR '1'='1", false, None);
        assert_eq!(w, "SITE_ADDR_STR LIKE 'X'' OR ''1''=''1%'");
    }
}
