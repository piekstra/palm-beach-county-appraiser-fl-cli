//! The county's per-parcel **detail page**, which carries everything the ArcGIS
//! layer does not: full sale history, ten years of appraisal/assessment/tax
//! rolls, itemized exemptions, and building structure (year built, square
//! footage).
//!
//! `pbcpao.gov/Property/Details?parcelId=<PCN17>` is a server-rendered page, but
//! it embeds its whole dataset as a single `var model = {…};` JSON literal in a
//! `<script>` tag. Reading that literal is far steadier than scraping the
//! rendered tables — it is the same object the page's own JavaScript binds to,
//! so it changes only when the site's data model does, not when its markup is
//! restyled.
//!
//! This is still a scrape, and it is treated as one: every field is optional and
//! parsing is best-effort. A missing field yields `None` rather than failing the
//! command. If the county renames something, fix the path and add a test — do
//! not make a field required.

use pk_cli_core::Money;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::AppError;
use crate::model::Sale;
use crate::pcn::Pcn;

/// Base URL of the county's public property site.
pub const SITE: &str = "https://pbcpao.gov";

/// The JS assignment that introduces the embedded dataset.
const MARKER: &str = "var model = ";

/// Building structure for a parcel (`building/v1`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Building {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year_built: Option<u32>,
    /// Heated/cooled living area in square feet — the county calls this "Area
    /// Under Air". This is the number people mean by "square footage".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub living_area_sqft: Option<u32>,
    /// Total area under roof: living area plus garage, porches, and other
    /// unconditioned space. Always ≥ `living_area_sqft`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_sqft: Option<u32>,
    /// Which building on the parcel this describes (parcels can have several).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub building_number: Option<String>,
    /// Every structural element the county publishes, verbatim.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub elements: Vec<Element>,
}

/// One `name: value` row of the county's structural-element table.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Element {
    pub name: String,
    pub value: String,
}

/// One tax year's valuation and levy (`tax-year/v1` rows).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct TaxYear {
    pub tax_year: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_total: Option<Money>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_land: Option<Money>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_improvement: Option<Money>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assessed: Option<Money>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exemption: Option<Money>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub taxable: Option<Money>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ad_valorem_tax: Option<Money>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub non_ad_valorem_tax: Option<Money>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_tax: Option<Money>,
}

/// An exemption on file, e.g. homestead.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Exemption {
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tax_year: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub applicant: Option<String>,
    /// Save Our Homes base year, when the exemption carries an SOH cap.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub soh_base_year: Option<String>,
}

/// Everything parsed out of one detail page.
#[derive(Debug, Clone, Default)]
pub struct Details {
    pub sales: Vec<Sale>,
    pub tax_years: Vec<TaxYear>,
    pub exemptions: Vec<Exemption>,
    pub building: Option<Building>,
}

/// Fetch and parse the detail page for `pcn`.
pub fn fetch(client: &reqwest::blocking::Client, pcn: &Pcn) -> Result<Details, AppError> {
    let url = format!("{SITE}/Property/Details?parcelId={}", pcn.bare());
    let resp = client.get(&url).send()?;
    let status = resp.status();
    if status.as_u16() == 404 {
        return Err(AppError::NotFound(format!(
            "no detail page for parcel {pcn}"
        )));
    }
    if !status.is_success() {
        return Err(AppError::Upstream(format!(
            "the county property site returned HTTP {} for parcel {pcn}",
            status.as_u16()
        )));
    }
    let html = resp
        .text()
        .map_err(|e| AppError::Upstream(format!("reading the detail page: {e}")))?;
    parse(&html)
}

/// Parse a detail page's HTML into [`Details`].
pub fn parse(html: &str) -> Result<Details, AppError> {
    let model = extract_model(html)?;
    Ok(Details {
        sales: sales(&model),
        tax_years: tax_years(&model),
        exemptions: exemptions(&model),
        building: building(&model),
    })
}

/// Pull the `var model = {…};` literal out of the page.
///
/// Brace-counting rather than a regex: the object contains braces inside string
/// values, so a lazy `\{.*?\}` match would truncate and a greedy one would run
/// past the end. The scanner tracks string state and escapes so only structural
/// braces are counted.
fn extract_model(html: &str) -> Result<Value, AppError> {
    let start = html.find(MARKER).map(|i| i + MARKER.len()).ok_or_else(|| {
        AppError::Upstream(
            "the county detail page no longer embeds a `var model` block — \
                 the site's markup has changed"
                .into(),
        )
    })?;
    let rest = &html[start..];
    if !rest.starts_with('{') {
        return Err(AppError::Upstream(
            "the county detail page's `var model` block is not a JSON object".into(),
        ));
    }

    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    let mut end = None;

    for (i, c) in rest.char_indices() {
        if in_string {
            match c {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match c {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(i + c.len_utf8());
                    break;
                }
            }
            _ => {}
        }
    }

    let end = end.ok_or_else(|| {
        AppError::Upstream("the county detail page's `var model` block is truncated".into())
    })?;
    serde_json::from_str(&rest[..end])
        .map_err(|e| AppError::Upstream(format!("parsing the detail page's data block: {e}")))
}

/// A trimmed string from a JSON object, or `None` when absent/blank/null.
fn field(v: &Value, key: &str) -> Option<String> {
    let s = v.get(key)?.as_str()?.trim();
    (!s.is_empty()).then(|| s.to_string())
}

/// A money field. The detail page publishes whole-dollar strings ("600000"),
/// sometimes with separators or a leading `$`.
fn money(v: &Value, key: &str) -> Option<Money> {
    Money::parse_usd(&field(v, key)?)
}

fn array<'a>(model: &'a Value, key: &str) -> &'a [Value] {
    model.get(key).and_then(Value::as_array).map_or(&[], |a| a)
}

/// Convert the page's `MM/DD/YYYY` dates to ISO `YYYY-MM-DD` (SPEC v1 §1.4).
/// Anything unrecognized passes through untouched rather than being dropped.
fn iso_date(raw: &str) -> String {
    let parts: Vec<&str> = raw.trim().split('/').collect();
    match parts.as_slice() {
        [m, d, y] if y.len() == 4 && m.len() <= 2 && d.len() <= 2 => {
            format!("{y}-{m:0>2}-{d:0>2}")
        }
        _ => raw.trim().to_string(),
    }
}

fn sales(model: &Value) -> Vec<Sale> {
    array(model, "salesInfo")
        .iter()
        .filter_map(|s| {
            let sale = Sale {
                date: field(s, "SaleDate").map(|d| iso_date(&d)),
                price: money(s, "Price"),
                book: field(s, "Book"),
                page: field(s, "Page"),
                instrument: field(s, "SaleType"),
                qualification_code: None,
                owner: field(s, "OwnerName"),
            };
            (!sale.is_empty()).then_some(sale)
        })
        .collect()
}

/// Merge the page's four parallel per-year tables (appraisal, assessment, tax)
/// into one row per tax year, newest first. They are published separately but
/// are keyed by the same `TaxYear`.
fn tax_years(model: &Value) -> Vec<TaxYear> {
    let mut years: Vec<TaxYear> = Vec::new();

    let row_for = |years: &mut Vec<TaxYear>, year: String| -> usize {
        match years.iter().position(|r| r.tax_year == year) {
            Some(i) => i,
            None => {
                years.push(TaxYear {
                    tax_year: year,
                    ..Default::default()
                });
                years.len() - 1
            }
        }
    };

    for a in array(model, "appraisalInfo") {
        let Some(year) = field(a, "TaxYear") else {
            continue;
        };
        let i = row_for(&mut years, year);
        years[i].market_total = money(a, "TotalMarketValue");
        years[i].market_land = money(a, "LandValue");
        years[i].market_improvement = money(a, "ImprovementValue");
    }
    for a in array(model, "assessmentInfo") {
        let Some(year) = field(a, "TaxYear") else {
            continue;
        };
        let i = row_for(&mut years, year);
        years[i].assessed = money(a, "AssessedValue");
        years[i].exemption = money(a, "ExemptionAmount");
        years[i].taxable = money(a, "TaxableValue");
    }
    for a in array(model, "taxInfo") {
        let Some(year) = field(a, "TaxYear") else {
            continue;
        };
        let i = row_for(&mut years, year);
        years[i].ad_valorem_tax = money(a, "AdValoremTax");
        years[i].non_ad_valorem_tax = money(a, "NonAdValoremTax");
        years[i].total_tax = money(a, "TotalTaxValue");
    }

    // Newest first, regardless of the order the tables arrived in.
    years.sort_by(|a, b| b.tax_year.cmp(&a.tax_year));
    years
}

fn exemptions(model: &Value) -> Vec<Exemption> {
    array(model, "exemptionInfo")
        .iter()
        .filter_map(|e| {
            Some(Exemption {
                description: field(e, "Description")?,
                tax_year: field(e, "TaxYear"),
                applicant: field(e, "ApplicantName"),
                soh_base_year: field(e, "SOHBaseYear"),
            })
        })
        .collect()
}

fn building(model: &Value) -> Option<Building> {
    let sd = model.get("structuralDetails")?;
    let elements: Vec<Element> = array(sd, "StructuralElements")
        .iter()
        .filter_map(|e| {
            Some(Element {
                name: field(e, "ElementName")?,
                value: field(e, "ElementValue")?,
            })
        })
        .collect();

    let lookup = |name: &str| -> Option<u32> {
        elements
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(name))
            .and_then(|e| e.value.replace(',', "").parse().ok())
    };

    let building = Building {
        year_built: lookup("Year Built"),
        // "Area Under Air" is the heated/cooled living area. The page's own
        // "Total Square Feet" is area under roof — it includes garage and
        // porches, so it is NOT living area despite the site's footnote
        // suggesting it may be.
        living_area_sqft: lookup("Area Under Air"),
        total_sqft: lookup("Total Square Footage"),
        building_number: array(sd, "BuildingNumbers")
            .first()
            .and_then(Value::as_str)
            .map(str::to_string),
        elements,
    };
    (building != Building::default()).then_some(building)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A synthetic page shaped like the county's, with the structures that
    /// actually matter: nested braces inside a string value, and the parallel
    /// per-year tables. No real parcel data (see AGENTS.md).
    fn page() -> String {
        let model = serde_json::json!({
            "propertyDetail": { "Location": "100 EXAMPLE ST {not a brace}" },
            "salesInfo": [
                {"SaleDate": "06/15/2020", "Price": "425000", "Book": "12345",
                 "Page": "00678", "SaleType": "WARRANTY DEED", "OwnerName": "DOE JANE"},
                {"SaleDate": "3/9/2011", "Price": "10", "Book": "23456",
                 "Page": "00042", "SaleType": "QUIT CLAIM", "OwnerName": ""}
            ],
            "appraisalInfo": [
                {"TaxYear": "2024", "ImprovementValue": "300000",
                 "LandValue": "250000", "TotalMarketValue": "550000"},
                {"TaxYear": "2025", "ImprovementValue": "320000",
                 "LandValue": "280000", "TotalMarketValue": "600000"}
            ],
            "assessmentInfo": [
                {"TaxYear": "2025", "TaxableValue": "350000",
                 "ExemptionAmount": "50000", "AssessedValue": "400000"}
            ],
            "taxInfo": [
                {"TaxYear": "2025", "AdValoremTax": "5800",
                 "NonAdValoremTax": "200", "TotalTaxValue": "6000"}
            ],
            "exemptionInfo": [
                {"Description": "HOMESTEAD", "TaxYear": "2026",
                 "ApplicantName": "DOE JANE", "SOHBaseYear": "2015"},
                {"Description": "", "TaxYear": "2026"}
            ],
            "structuralDetails": {
                "BuildingNumbers": ["1"],
                "StructuralElements": [
                    {"ElementName": "Year Built", "ElementValue": "1990"},
                    {"ElementName": "Total Square Footage", "ElementValue": "2800"},
                    {"ElementName": "Area Under Air", "ElementValue": "2000"},
                    {"ElementName": "Exterior Wall 1", "ElementValue": "CB STUCCO"},
                    {"ElementName": "Blank", "ElementValue": ""}
                ]
            }
        });
        format!(
            "<html><body><script>\n  var model = {};\n</script></body></html>",
            serde_json::to_string(&model).unwrap()
        )
    }

    #[test]
    fn extracts_the_embedded_model_past_braces_in_strings() {
        let v = extract_model(&page()).unwrap();
        // The brace inside a string value must not terminate the scan.
        assert_eq!(
            v["propertyDetail"]["Location"],
            "100 EXAMPLE ST {not a brace}"
        );
        assert!(v["salesInfo"].is_array());
    }

    #[test]
    fn reports_a_markup_change_instead_of_panicking() {
        let err = parse("<html><body>no data here</body></html>").unwrap_err();
        assert_eq!(err.exit_code(), 5, "a changed site is an upstream failure");
        assert!(err.to_string().contains("var model"));
    }

    #[test]
    fn reports_a_truncated_model_block() {
        let err = parse("<script>var model = {\"a\": 1</script>").unwrap_err();
        assert_eq!(err.exit_code(), 5);
    }

    #[test]
    fn parses_full_sale_history_with_iso_dates() {
        let d = parse(&page()).unwrap();
        assert_eq!(d.sales.len(), 2, "every recorded sale, not just the latest");
        assert_eq!(d.sales[0].date.as_deref(), Some("2020-06-15"));
        assert_eq!(d.sales[0].owner.as_deref(), Some("DOE JANE"));
        assert_eq!(d.sales[0].price.as_ref().unwrap().amount, "425000.00");
        // Single-digit month/day still normalize to ISO.
        assert_eq!(d.sales[1].date.as_deref(), Some("2011-03-09"));
        assert_eq!(d.sales[1].price.as_ref().unwrap().amount, "10.00");
        // A blank owner is dropped rather than emitted as an empty string.
        assert!(d.sales[1].owner.is_none());
    }

    #[test]
    fn merges_parallel_year_tables_newest_first() {
        let d = parse(&page()).unwrap();
        assert_eq!(d.tax_years.len(), 2);
        let latest = &d.tax_years[0];
        assert_eq!(latest.tax_year, "2025");
        assert_eq!(latest.market_total.as_ref().unwrap().amount, "600000.00");
        assert_eq!(latest.assessed.as_ref().unwrap().amount, "400000.00");
        assert_eq!(latest.total_tax.as_ref().unwrap().amount, "6000.00");
        // 2024 appears in the appraisal table only; its other columns stay absent
        // rather than being invented.
        assert_eq!(d.tax_years[1].tax_year, "2024");
        assert!(d.tax_years[1].total_tax.is_none());
    }

    #[test]
    fn reads_building_structure() {
        let b = parse(&page()).unwrap().building.unwrap();
        assert_eq!(b.year_built, Some(1990));
        // Living area is "Area Under Air" — not the larger under-roof total.
        assert_eq!(b.living_area_sqft, Some(2000));
        assert_eq!(b.total_sqft, Some(2800));
        assert!(b.total_sqft > b.living_area_sqft);
        assert_eq!(b.building_number.as_deref(), Some("1"));
        // Blank-valued elements are dropped; real ones are kept verbatim.
        assert!(b.elements.iter().all(|e| !e.value.is_empty()));
        assert!(b.elements.iter().any(|e| e.name == "Exterior Wall 1"));
    }

    #[test]
    fn skips_exemption_rows_without_a_description() {
        let d = parse(&page()).unwrap();
        assert_eq!(d.exemptions.len(), 1);
        assert_eq!(d.exemptions[0].description, "HOMESTEAD");
        assert_eq!(d.exemptions[0].soh_base_year.as_deref(), Some("2015"));
    }

    #[test]
    fn missing_sections_yield_empty_not_an_error() {
        // A vacant lot has no building and no sales; that is data, not failure.
        let d = parse("<script>var model = {};</script>").unwrap();
        assert!(d.sales.is_empty());
        assert!(d.tax_years.is_empty());
        assert!(d.exemptions.is_empty());
        assert!(d.building.is_none());
    }

    #[test]
    fn passes_through_unrecognized_date_formats() {
        assert_eq!(iso_date("2025-01-03"), "2025-01-03");
        assert_eq!(iso_date(""), "");
    }
}
