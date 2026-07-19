//! Normalized parcel records built from raw ArcGIS `attributes` objects.
//!
//! The layer's columns are terse, inconsistently typed (`PRICE` is a *string*,
//! `SALE_DATE` is epoch **milliseconds**), and liberally sprinkled with blanks.
//! Everything crossing into the DTOs below is converted to the family output
//! contract (SPEC v1 §1.4): `snake_case` keys, ISO `YYYY-MM-DD` dates, money as
//! `{"amount","currency"}` string decimals — never floats — and absent fields
//! omitted rather than emitted as null noise.

use pk_cli_core::Money;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::pcn::Pcn;
use crate::util::iso_from_epoch_ms;

/// One row of a search result — the light projection listed by `search`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParcelSummary {
    pub pcn: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub situs_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub municipality: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_value: Option<Money>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assessed_value: Option<Money>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub property_use: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_sale: Option<Sale>,
    /// Present only when the owner is statutorily confidential (`CONFID_FLG`).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub confidential: bool,
}

/// A recorded sale. The ArcGIS layer carries only the most recent one; the
/// county's detail page carries the full history (see [`crate::details`]).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Sale {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<Money>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub book: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
    /// Deed type — the terse code (`WD`) from ArcGIS, or the spelled-out form
    /// (`WARRANTY DEED`) from the detail page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instrument: Option<String>,
    /// The county's sale-qualification code: whether the sale is treated as an
    /// arm's-length market transaction.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qualification_code: Option<String>,
    /// Buyer of record for this sale (detail page only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
}

impl Sale {
    /// True when every field is blank, i.e. the parcel has no recorded sale.
    pub fn is_empty(&self) -> bool {
        *self == Sale::default()
    }
}

/// Mailing address of record for the owner.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct MailingAddress {
    /// Street lines (`PADDR1`..`PADDR3`), blanks dropped.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub lines: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zip: Option<String>,
}

impl MailingAddress {
    pub fn is_empty(&self) -> bool {
        *self == MailingAddress::default()
    }

    /// Single-line rendering for text output.
    pub fn one_line(&self) -> String {
        let mut parts = self.lines.clone();
        let tail: Vec<String> = [&self.city, &self.state, &self.zip]
            .into_iter()
            .flatten()
            .cloned()
            .collect();
        if !tail.is_empty() {
            parts.push(tail.join(" "));
        }
        parts.join(", ")
    }
}

/// The appraiser's valuation of a parcel for the current roll.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Values {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_total: Option<Money>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_land: Option<Money>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_improvement: Option<Money>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assessed: Option<Money>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub taxable: Option<Money>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exemption: Option<Money>,
    /// `true` when a homestead exemption is on file (`HMSTD_FLG`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub homestead: Option<bool>,
}

/// The full record for one parcel (`parcel/v1`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Parcel {
    pub schema: String,
    /// Dashed display form, `12-34-56-78-90-123-4567`.
    pub pcn: String,
    /// Bare 17-digit form as stored in `PARID` — what other commands take.
    pub pcn_bare: String,
    /// `OWNER_NAME1` and `OWNER_NAME2`, blanks dropped. Surname-first.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub owners: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub situs_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub municipality: Option<String>,
    #[serde(skip_serializing_if = "MailingAddress::is_empty")]
    pub mailing_address: MailingAddress,
    pub values: Values,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_sale: Option<Sale>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub property_use: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subdivision: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acres: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub legal_description: Option<String>,
    /// `lat,lng` centroid as published by the county.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coordinates: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub confidential: bool,
    /// Building details, present only with `parcel --details` (which costs a
    /// second request against the county's detail page).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub building: Option<crate::details::Building>,
}

/// A schema-tagged list of parcels (`parcel-list/v1`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParcelList {
    pub schema: String,
    /// Rows returned, after `--limit`.
    pub items: Vec<ParcelSummary>,
    /// Total rows matching the query server-side, which may exceed `items`.
    pub total: u64,
    /// True when `total` exceeds the number of rows returned.
    pub truncated: bool,
}

impl ParcelList {
    pub fn new(items: Vec<ParcelSummary>, total: u64) -> Self {
        let truncated = total > items.len() as u64;
        ParcelList {
            schema: "parcel-list/v1".into(),
            items,
            total,
            truncated,
        }
    }
}

// ---------------------------------------------------------------------------
// Attribute extraction
// ---------------------------------------------------------------------------

/// A trimmed string field, or `None` when absent/blank. The layer uses empty
/// strings and whitespace interchangeably with nulls.
pub fn text(attrs: &Value, key: &str) -> Option<String> {
    let s = match attrs.get(key)? {
        Value::String(s) => s.trim().to_string(),
        Value::Number(n) => n.to_string(),
        _ => return None,
    };
    (!s.is_empty()).then_some(s)
}

/// A numeric field as `Money`. ArcGIS stores these as doubles; they become
/// two-decimal strings so no float ever reaches the output.
pub fn money(attrs: &Value, key: &str) -> Option<Money> {
    match attrs.get(key)? {
        Value::Number(n) => n.as_f64().map(|v| Money::usd(format!("{v:.2}"))),
        // `PRICE` is a string in this layer — parse it like a provider amount.
        Value::String(s) => Money::parse_usd(s),
        _ => None,
    }
}

/// An epoch-millisecond date field as ISO `YYYY-MM-DD`.
pub fn date(attrs: &Value, key: &str) -> Option<String> {
    attrs.get(key)?.as_i64().and_then(iso_from_epoch_ms)
}

/// A `Y`/`N` flag as a bool. Anything else is treated as absent.
pub fn yes_no(attrs: &Value, key: &str) -> Option<bool> {
    match text(attrs, key)?.to_uppercase().as_str() {
        "Y" => Some(true),
        "N" => Some(false),
        _ => None,
    }
}

fn sale_from_attrs(attrs: &Value) -> Option<Sale> {
    let sale = Sale {
        date: date(attrs, "SALE_DATE"),
        price: money(attrs, "PRICE"),
        book: text(attrs, "BOOK"),
        page: text(attrs, "PAGE"),
        instrument: text(attrs, "INSTRUMENT"),
        qualification_code: text(attrs, "QUAL_CODE"),
        owner: None,
    };
    (!sale.is_empty()).then_some(sale)
}

/// `OWNER_NAME1` plus `OWNER_NAME2`, joined for one-line display.
///
/// The county appends a trailing `&` to the first name to mean "and others"
/// (`DOE JANE &`). Joining those verbatim would render `... & & ...`, so
/// the marker is dropped here — the `owners` array keeps the raw values.
fn owner_line(attrs: &Value) -> Option<String> {
    let names: Vec<String> = owners(attrs)
        .into_iter()
        .map(|n| n.trim_end_matches('&').trim_end().to_string())
        .filter(|n| !n.is_empty())
        .collect();
    (!names.is_empty()).then(|| names.join(" & "))
}

fn owners(attrs: &Value) -> Vec<String> {
    ["OWNER_NAME1", "OWNER_NAME2"]
        .iter()
        .filter_map(|k| text(attrs, k))
        .collect()
}

/// The owner's mailing address.
///
/// Only `PADDR1`/`PADDR2` are street lines. **`PADDR3` is not** — it is the
/// county's pre-formatted `CITY STATE ZIP` line, duplicating `CITYNAME`,
/// `STATE`, and `ZIP1`. Treating it as a third street line prints the city
/// twice, so it is used only as a fallback when the structured fields are all
/// absent.
fn mailing(attrs: &Value) -> MailingAddress {
    let mut lines: Vec<String> = ["PADDR1", "PADDR2"]
        .iter()
        .filter_map(|k| text(attrs, k))
        .collect();

    let (city, state, zip) = (text(attrs, "CITYNAME"), text(attrs, "STATE"), zip(attrs));
    if city.is_none() && state.is_none() && zip.is_none() {
        if let Some(formatted) = text(attrs, "PADDR3") {
            lines.push(formatted);
        }
    }

    MailingAddress {
        lines,
        city,
        state,
        zip,
    }
}

/// `ZIP1` optionally extended by the `+4` in `ZIP2`.
fn zip(attrs: &Value) -> Option<String> {
    let base = text(attrs, "ZIP1")?;
    Some(match text(attrs, "ZIP2") {
        Some(plus4) => format!("{base}-{plus4}"),
        None => base,
    })
}

/// The legal description, split across three columns that are often blank.
fn legal(attrs: &Value) -> Option<String> {
    let parts: Vec<String> = ["LEGAL1", "LEGAL2", "LEGAL3"]
        .iter()
        .filter_map(|k| text(attrs, k))
        .collect();
    (!parts.is_empty()).then(|| parts.join(" "))
}

impl ParcelSummary {
    /// Build a search row from a raw ArcGIS `attributes` object.
    pub fn from_attrs(attrs: &Value) -> Option<Self> {
        let parid = text(attrs, "PARID")?;
        Some(ParcelSummary {
            pcn: Pcn::parse(&parid).map(|p| p.dashed()).unwrap_or(parid),
            owner: owner_line(attrs),
            situs_address: text(attrs, "SITE_ADDR_STR"),
            municipality: text(attrs, "MUNICIPALITY"),
            market_value: money(attrs, "TOTAL_MARKET"),
            assessed_value: money(attrs, "ASSESSED_VAL"),
            property_use: text(attrs, "PROPERTY_USE"),
            last_sale: sale_from_attrs(attrs),
            confidential: yes_no(attrs, "CONFID_FLG").unwrap_or(false),
        })
    }
}

impl Parcel {
    /// Build the full record from a raw ArcGIS `attributes` object.
    pub fn from_attrs(attrs: &Value) -> Option<Self> {
        let parid = text(attrs, "PARID").or_else(|| text(attrs, "PARCEL_NUMBER"))?;
        let pcn = Pcn::parse(&parid).ok();

        Some(Parcel {
            schema: "parcel/v1".into(),
            pcn: pcn
                .as_ref()
                .map(Pcn::dashed)
                .unwrap_or_else(|| parid.clone()),
            pcn_bare: pcn
                .as_ref()
                .map(|p| p.bare().to_string())
                .unwrap_or_else(|| parid.clone()),
            owners: owners(attrs),
            situs_address: text(attrs, "SITE_ADDR_STR"),
            municipality: text(attrs, "MUNICIPALITY"),
            mailing_address: mailing(attrs),
            values: Values {
                market_total: money(attrs, "TOTAL_MARKET"),
                market_land: money(attrs, "LAND_MARKET"),
                market_improvement: money(attrs, "IMPRV_MRKT"),
                assessed: money(attrs, "ASSESSED_VAL"),
                taxable: money(attrs, "TOTAL_TAXABLE"),
                exemption: money(attrs, "EXEMPTION"),
                homestead: yes_no(attrs, "HMSTD_FLG"),
            },
            last_sale: sale_from_attrs(attrs),
            property_use: text(attrs, "PROPERTY_USE"),
            subdivision: text(attrs, "SUBDIV_NAME"),
            acres: attrs
                .get("ACRES")
                .and_then(Value::as_f64)
                .filter(|a| *a > 0.0),
            legal_description: legal(attrs),
            coordinates: text(attrs, "LATLNG"),
            confidential: yes_no(attrs, "CONFID_FLG").unwrap_or(false),
            building: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A synthetic feature shaped like the real layer. No real parcel data ever
    /// lands in this repo (see AGENTS.md).
    fn sample() -> Value {
        json!({
            "PARID": "12345678901234567",
            "OWNER_NAME1": "DOE JANE",
            "OWNER_NAME2": "DOE JOHN",
            "SITE_ADDR_STR": "100 EXAMPLE ST",
            "MUNICIPALITY": "TEST CITY",
            "PADDR1": "PO BOX 1",
            "PADDR2": "  ",
            "PADDR3": "C/O EXAMPLE TRUST",
            "CITYNAME": "TEST CITY",
            "STATE": "FL",
            "ZIP1": "33400",
            "ZIP2": "1234",
            "TOTAL_MARKET": 500000.0,
            "LAND_MARKET": 200000.0,
            "IMPRV_MRKT": 300000.0,
            "ASSESSED_VAL": 350000.5,
            "TOTAL_TAXABLE": 300000.0,
            "EXEMPTION": 50000.0,
            "HMSTD_FLG": "Y",
            "SALE_DATE": 1_592_179_200_000i64,
            "PRICE": "425000",
            "BOOK": "12345",
            "PAGE": "0678",
            "INSTRUMENT": "WD",
            "QUAL_CODE": "QM",
            "PROPERTY_USE": "0100",
            "SUBDIV_NAME": "EXAMPLE SUB",
            "ACRES": 0.25,
            "LEGAL1": "EXAMPLE SUB",
            "LEGAL2": "LT 58",
            "LEGAL3": "",
            "LATLNG": "26.9,-80.1",
            "CONFID_FLG": "N"
        })
    }

    #[test]
    fn builds_a_full_parcel() {
        let p = Parcel::from_attrs(&sample()).unwrap();
        assert_eq!(p.schema, "parcel/v1");
        assert_eq!(p.pcn, "12-34-56-78-90-123-4567");
        assert_eq!(p.pcn_bare, "12345678901234567");
        assert_eq!(p.owners, vec!["DOE JANE", "DOE JOHN"]);
        assert_eq!(p.subdivision.as_deref(), Some("EXAMPLE SUB"));
        assert_eq!(p.acres, Some(0.25));
        assert!(!p.confidential);
    }

    #[test]
    fn money_never_serializes_as_a_float() {
        let p = Parcel::from_attrs(&sample()).unwrap();
        let v = serde_json::to_value(&p).unwrap();
        let amount = &v["values"]["market_total"]["amount"];
        assert!(amount.is_string(), "money must be a string decimal");
        assert_eq!(amount, "500000.00");
        assert_eq!(v["values"]["market_total"]["currency"], "USD");
        // A fractional double rounds to two places rather than leaking `.5`.
        assert_eq!(v["values"]["assessed"]["amount"], "350000.50");
    }

    #[test]
    fn price_string_field_parses_as_money() {
        let sale = Parcel::from_attrs(&sample()).unwrap().last_sale.unwrap();
        assert_eq!(sale.price.unwrap().amount, "425000.00");
        // Epoch milliseconds become an ISO date.
        assert_eq!(sale.date.as_deref(), Some("2020-06-15"));
    }

    #[test]
    fn blank_fields_are_dropped_not_emitted_as_null() {
        let p = Parcel::from_attrs(&sample()).unwrap();
        // PADDR2 was whitespace and LEGAL3 empty — neither should survive.
        assert_eq!(p.mailing_address.lines, vec!["PO BOX 1"]);
        assert_eq!(p.legal_description.as_deref(), Some("EXAMPLE SUB LT 58"));

        let v = serde_json::to_value(&p).unwrap();
        assert!(
            v.as_object().unwrap().values().all(|x| !x.is_null()),
            "no null noise: {v}"
        );
    }

    #[test]
    fn combines_zip_plus_four() {
        let p = Parcel::from_attrs(&sample()).unwrap();
        assert_eq!(p.mailing_address.zip.as_deref(), Some("33400-1234"));
        assert_eq!(
            p.mailing_address.one_line(),
            "PO BOX 1, TEST CITY FL 33400-1234"
        );
    }

    #[test]
    fn does_not_repeat_the_city_line_from_paddr3() {
        // PADDR3 is the county's pre-formatted "CITY STATE ZIP" line. Treating
        // it as a street line printed the city twice.
        let mut attrs = sample();
        attrs["PADDR3"] = json!("TEST CITY FL 33400");
        let m = Parcel::from_attrs(&attrs).unwrap().mailing_address;
        assert_eq!(m.lines, vec!["PO BOX 1"]);
        assert_eq!(m.one_line(), "PO BOX 1, TEST CITY FL 33400-1234");
    }

    #[test]
    fn falls_back_to_paddr3_when_structured_fields_are_absent() {
        let mut attrs = sample();
        for k in ["CITYNAME", "STATE", "ZIP1", "ZIP2"] {
            attrs[k] = json!(null);
        }
        attrs["PADDR3"] = json!("TEST CITY FL 33400");
        let m = Parcel::from_attrs(&attrs).unwrap().mailing_address;
        assert_eq!(m.one_line(), "PO BOX 1, TEST CITY FL 33400");
    }

    #[test]
    fn owner_line_drops_the_counties_et_al_ampersand() {
        // `OWNER_NAME1` ends in `&` to mean "and others"; joining verbatim
        // rendered "DOE JANE & & DOE JOHN".
        let mut attrs = sample();
        attrs["OWNER_NAME1"] = json!("DOE JANE &");
        let s = ParcelSummary::from_attrs(&attrs).unwrap();
        assert_eq!(s.owner.as_deref(), Some("DOE JANE & DOE JOHN"));
        // The raw values stay faithful to the provider in the owners array.
        assert_eq!(
            Parcel::from_attrs(&attrs).unwrap().owners,
            vec!["DOE JANE &", "DOE JOHN"]
        );
    }

    #[test]
    fn sole_owner_with_a_trailing_ampersand_renders_cleanly() {
        let mut attrs = sample();
        attrs["OWNER_NAME1"] = json!("DOE JANE &");
        attrs["OWNER_NAME2"] = json!(null);
        let s = ParcelSummary::from_attrs(&attrs).unwrap();
        assert_eq!(s.owner.as_deref(), Some("DOE JANE"));
    }

    #[test]
    fn homestead_flag_maps_to_bool() {
        assert_eq!(
            Parcel::from_attrs(&sample()).unwrap().values.homestead,
            Some(true)
        );
        let mut attrs = sample();
        attrs["HMSTD_FLG"] = json!("N");
        assert_eq!(
            Parcel::from_attrs(&attrs).unwrap().values.homestead,
            Some(false)
        );
    }

    #[test]
    fn flags_confidential_owners() {
        let mut attrs = sample();
        attrs["CONFID_FLG"] = json!("Y");
        let p = Parcel::from_attrs(&attrs).unwrap();
        assert!(p.confidential);
        // Serialized only when true, so ordinary parcels stay uncluttered.
        assert_eq!(
            serde_json::to_value(&p).unwrap()["confidential"],
            json!(true)
        );
    }

    #[test]
    fn parcel_without_a_recorded_sale_omits_it() {
        let mut attrs = sample();
        for k in [
            "SALE_DATE",
            "PRICE",
            "BOOK",
            "PAGE",
            "INSTRUMENT",
            "QUAL_CODE",
        ] {
            attrs[k] = json!(null);
        }
        assert!(Parcel::from_attrs(&attrs).unwrap().last_sale.is_none());
    }

    #[test]
    fn requires_a_parcel_id() {
        let mut attrs = sample();
        attrs["PARID"] = json!(null);
        assert!(ParcelSummary::from_attrs(&attrs).is_none());
        // The full record falls back to PARCEL_NUMBER when PARID is absent.
        attrs["PARCEL_NUMBER"] = json!("12345678901234567");
        assert!(Parcel::from_attrs(&attrs).is_some());
    }

    #[test]
    fn summary_joins_both_owner_names() {
        let s = ParcelSummary::from_attrs(&sample()).unwrap();
        assert_eq!(s.owner.as_deref(), Some("DOE JANE & DOE JOHN"));
        assert_eq!(s.situs_address.as_deref(), Some("100 EXAMPLE ST"));
    }

    #[test]
    fn list_computes_truncation_from_the_server_total() {
        let one = vec![ParcelSummary::from_attrs(&sample()).unwrap()];
        let list = ParcelList::new(one.clone(), 37);
        assert_eq!(list.schema, "parcel-list/v1");
        assert!(list.truncated);
        assert!(!ParcelList::new(one, 1).truncated);
    }
}
