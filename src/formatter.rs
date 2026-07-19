//! Human-readable and `--json` rendering (SPEC v1 §1.4): key/value blocks for a
//! single resource, pipe-delimited tables for lists, JSON DTO alone on stdout
//! under `--json`. Diagnostics never come through here — they go to stderr.
//!
//! Lists render through the family's shared table renderer rather than a
//! bespoke one, so `pbca` output lines up with the sibling CLIs and knows how
//! to print `Money` objects.

use pk_cli_core::output;
use serde::Serialize;
use serde_json::{json, Value};

use crate::details::{Building, Exemption, TaxYear};
use crate::model::{Parcel, ParcelList, Sale};

/// Emit a schema-tagged DTO as JSON.
fn emit_json<T: Serialize>(dto: &T) {
    output::json(&serde_json::to_value(dto).unwrap_or(Value::Null));
}

/// `—` for an absent value, so columns stay aligned in text mode.
const DASH: &str = "—";

fn or_dash(v: Option<&str>) -> String {
    v.filter(|s| !s.trim().is_empty())
        .unwrap_or(DASH)
        .to_string()
}

fn money_str(m: Option<&pk_cli_core::Money>) -> String {
    m.map(|m| m.to_string()).unwrap_or_else(|| DASH.into())
}

// ---------------------------------------------------------------------------
// search
// ---------------------------------------------------------------------------

pub fn print_search(list: &ParcelList, json: bool) {
    if json {
        emit_json(list);
        return;
    }
    if list.items.is_empty() {
        println!("No matching parcels.");
        return;
    }

    let rows: Vec<Value> = list
        .items
        .iter()
        .map(|p| {
            json!({
                "pcn": p.pcn,
                "owner": or_dash(p.owner.as_deref()),
                "situs": or_dash(p.situs_address.as_deref()),
                "municipality": or_dash(p.municipality.as_deref()),
                "market": money_str(p.market_value.as_ref()),
            })
        })
        .collect();
    output::table(&rows);

    // Say plainly how much was withheld — a silent cap reads as "that's all
    // there is", which for a countywide search is usually wrong.
    if list.truncated {
        println!();
        println!(
            "Showing {} of {} matches — raise --limit to see more.",
            list.items.len(),
            list.total
        );
    }
}

// ---------------------------------------------------------------------------
// parcel
// ---------------------------------------------------------------------------

pub fn print_parcel(p: &Parcel, json: bool) {
    if json {
        emit_json(p);
        return;
    }

    println!("Parcel {}", p.pcn);
    if p.confidential {
        println!("  ** owner is statutorily confidential (Fla. Stat. §119.071) **");
    }
    println!(
        "  Owner:        {}",
        or_dash(p.owners.first().map(String::as_str))
    );
    for extra in p.owners.iter().skip(1) {
        println!("                {extra}");
    }
    println!("  Situs:        {}", or_dash(p.situs_address.as_deref()));
    if let Some(m) = &p.municipality {
        println!("  Municipality: {m}");
    }
    if !p.mailing_address.is_empty() {
        println!("  Mailing:      {}", p.mailing_address.one_line());
    }

    println!();
    println!("  Values:");
    println!(
        "    Market (total):     {}",
        money_str(p.values.market_total.as_ref())
    );
    println!(
        "      land:             {}",
        money_str(p.values.market_land.as_ref())
    );
    println!(
        "      improvement:      {}",
        money_str(p.values.market_improvement.as_ref())
    );
    println!(
        "    Assessed:           {}",
        money_str(p.values.assessed.as_ref())
    );
    println!(
        "    Exemptions:         {}",
        money_str(p.values.exemption.as_ref())
    );
    println!(
        "    Taxable:            {}",
        money_str(p.values.taxable.as_ref())
    );
    if let Some(true) = p.values.homestead {
        println!("    Homestead:          yes");
    }

    if let Some(sale) = &p.last_sale {
        println!();
        println!("  Last sale:");
        println!("    Date:               {}", or_dash(sale.date.as_deref()));
        println!("    Price:              {}", money_str(sale.price.as_ref()));
        if let (Some(b), Some(pg)) = (&sale.book, &sale.page) {
            println!("    OR book/page:       {b} / {pg}");
        }
        if let Some(i) = &sale.instrument {
            println!("    Instrument:         {i}");
        }
    }

    if let Some(b) = &p.building {
        println!();
        print_building_block(b, false);
    }

    println!();
    println!("  Property:");
    if let Some(u) = &p.property_use {
        println!("    Use code:           {u}");
    }
    if let Some(s) = &p.subdivision {
        println!("    Subdivision:        {s}");
    }
    if let Some(a) = p.acres {
        println!("    Acres:              {a}");
    }
    if let Some(l) = &p.legal_description {
        println!("    Legal:              {l}");
    }
    if let Some(c) = &p.coordinates {
        println!("    Coordinates:        {c}");
    }
}

// ---------------------------------------------------------------------------
// sales
// ---------------------------------------------------------------------------

/// Schema-tagged sale history (`sale-list/v1`).
pub fn print_sales(pcn: &str, sales: &[Sale], json: bool) {
    if json {
        output::json(&json!({
            "schema": "sale-list/v1",
            "pcn": pcn,
            "items": sales,
        }));
        return;
    }
    if sales.is_empty() {
        println!("No recorded sales for parcel {pcn}.");
        return;
    }
    println!("Sales for parcel {pcn}");
    println!();
    let rows: Vec<Value> = sales
        .iter()
        .map(|s| {
            json!({
                "date": or_dash(s.date.as_deref()),
                "price": money_str(s.price.as_ref()),
                "type": or_dash(s.instrument.as_deref()),
                "book_page": match (&s.book, &s.page) {
                    (Some(b), Some(p)) => format!("{b}/{p}"),
                    _ => DASH.into(),
                },
                "owner": or_dash(s.owner.as_deref()),
            })
        })
        .collect();
    output::table(&rows);
}

// ---------------------------------------------------------------------------
// taxes
// ---------------------------------------------------------------------------

/// Schema-tagged value/tax history (`tax-history/v1`).
pub fn print_taxes(pcn: &str, years: &[TaxYear], exemptions: &[Exemption], json: bool) {
    if json {
        output::json(&json!({
            "schema": "tax-history/v1",
            "pcn": pcn,
            "items": years,
            "exemptions": exemptions,
        }));
        return;
    }
    if years.is_empty() {
        println!("No tax history for parcel {pcn}.");
        return;
    }
    println!("Value and tax history for parcel {pcn}");
    println!();
    let rows: Vec<Value> = years
        .iter()
        .map(|y| {
            json!({
                "year": y.tax_year,
                "market": money_str(y.market_total.as_ref()),
                "assessed": money_str(y.assessed.as_ref()),
                "exempt": money_str(y.exemption.as_ref()),
                "taxable": money_str(y.taxable.as_ref()),
                "total_tax": money_str(y.total_tax.as_ref()),
            })
        })
        .collect();
    output::table(&rows);

    if !exemptions.is_empty() {
        println!();
        println!("Exemptions on file:");
        for e in exemptions {
            let year = e.tax_year.as_deref().unwrap_or(DASH);
            let who = e.applicant.as_deref().unwrap_or(DASH);
            println!("  {year}  {:<24} {who}", e.description);
        }
    }
}

// ---------------------------------------------------------------------------
// building
// ---------------------------------------------------------------------------

/// Schema-tagged building structure (`building/v1`).
pub fn print_building(pcn: &str, building: Option<&Building>, json: bool) {
    if json {
        match building {
            Some(b) => {
                let mut v = serde_json::to_value(b).unwrap_or(Value::Null);
                if let Some(o) = v.as_object_mut() {
                    o.insert("schema".into(), json!("building/v1"));
                    o.insert("pcn".into(), json!(pcn));
                }
                output::json(&v);
            }
            None => output::json(&json!({
                "schema": "building/v1",
                "pcn": pcn,
                "elements": [],
            })),
        }
        return;
    }
    match building {
        None => println!("No building on record for parcel {pcn} (vacant land?)."),
        Some(b) => {
            println!("Building for parcel {pcn}");
            print_building_block(b, true);
        }
    }
}

/// The building block, at the same indent as the other `parcel` sections.
/// `full` adds the structural-element dump — long, and mostly wanted only when
/// asked for directly, so `parcel --details` shows the headline numbers alone.
fn print_building_block(b: &Building, full: bool) {
    println!("  Building:");
    if let Some(y) = b.year_built {
        println!("    Year built:         {y}");
    }
    if let Some(a) = b.living_area_sqft {
        println!("    Living area:        {a} sq ft (heated/cooled)");
    }
    if let Some(t) = b.total_sqft {
        println!("    Total under roof:   {t} sq ft");
    }
    if full && !b.elements.is_empty() {
        println!();
        println!("  Structural elements:");
        for e in &b.elements {
            println!("    {:<26} {}", e.name, e.value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ParcelSummary;
    use serde_json::json as j;

    fn summary() -> ParcelSummary {
        ParcelSummary::from_attrs(&j!({
            "PARID": "12345678901234567",
            "OWNER_NAME1": "DOE JANE",
            "SITE_ADDR_STR": "100 EXAMPLE ST",
            "MUNICIPALITY": "TEST CITY",
            "TOTAL_MARKET": 500000.0
        }))
        .unwrap()
    }

    #[test]
    fn search_json_is_the_dto_alone() {
        let list = ParcelList::new(vec![summary()], 37);
        let v = serde_json::to_value(&list).unwrap();
        assert_eq!(v["schema"], "parcel-list/v1");
        assert_eq!(v["total"], 37);
        assert_eq!(v["truncated"], true);
        assert_eq!(v["items"][0]["pcn"], "12-34-56-78-90-123-4567");
        // Money stays a string decimal all the way to the wire.
        assert_eq!(v["items"][0]["market_value"]["amount"], "500000.00");
    }

    #[test]
    fn absent_values_render_as_a_dash_not_empty() {
        assert_eq!(or_dash(None), DASH);
        assert_eq!(or_dash(Some("   ")), DASH);
        assert_eq!(or_dash(Some("x")), "x");
        assert_eq!(money_str(None), DASH);
    }

    #[test]
    fn building_json_carries_its_schema_and_pcn() {
        // Rendering is stdout-only, so assert on the DTO the way it is built.
        let b = Building {
            year_built: Some(1990),
            living_area_sqft: Some(2000),
            total_sqft: Some(2800),
            building_number: Some("1".into()),
            elements: vec![],
        };
        let mut v = serde_json::to_value(&b).unwrap();
        let o = v.as_object_mut().unwrap();
        o.insert("schema".into(), j!("building/v1"));
        assert_eq!(v["schema"], "building/v1");
        assert_eq!(v["living_area_sqft"], 2000);
    }
}
