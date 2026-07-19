//! Thin, polite client over the Palm Beach County enterprise **ArcGIS** server
//! that hosts the Property Appraiser's parcel data.
//!
//! Everything here is anonymous public records: no API key, no token, no login,
//! and — unlike the district portals the sibling CLIs talk to — no WAF, so a
//! plain client User-Agent is fine.
//!
//! Two ArcGIS behaviours shape this module:
//! * **Errors arrive as HTTP 200.** A bad `where` clause returns
//!   `{"error": {"code": 400, "message": ...}}` with a success status, so the
//!   body must be inspected rather than trusting the status code.
//! * **Results are capped** at `maxRecordCount` (5000) per request. A truncated
//!   response sets `exceededTransferLimit`, and the next page is fetched with
//!   `resultOffset`. [`Arcgis::query_all`] implements that loop.

use pk_cli_core::CliError;
use serde_json::Value;

use crate::util::sql_escape;

/// The ArcGIS services folder holding the appraiser's layers. Also the base the
/// `pbca api` passthrough resolves relative paths against.
pub const BASE: &str = "https://gis.pbcgov.org/arcgis/rest/services/Parcels";

/// `PARCEL_INFO` layer 4 (`PARCEL_DETAILS`) — one row per parcel, ~685k rows
/// countywide, carrying owner, mailing, situs, values, exemptions, last sale.
pub const PARCEL_LAYER: &str = "/PARCEL_INFO/FeatureServer/4";

/// Server-side cap on rows per request (`maxRecordCount` on the layer).
pub const MAX_RECORD_COUNT: u32 = 5000;

/// The columns a search needs — a light projection, so listing 50 matches
/// doesn't drag ~58 columns per row across the wire. `parcel` fetches `*`.
pub const SEARCH_FIELDS: &str = "PARID,OWNER_NAME1,OWNER_NAME2,SITE_ADDR_STR,MUNICIPALITY,\
                                 TOTAL_MARKET,ASSESSED_VAL,PROPERTY_USE,SALE_DATE,PRICE,CONFID_FLG";

/// Client bound to the appraiser's ArcGIS service folder.
pub struct Arcgis {
    client: reqwest::blocking::Client,
    base: String,
}

impl Arcgis {
    pub fn new() -> Result<Self, CliError> {
        Ok(Self {
            client: pk_cli_http::client("pbca", env!("CARGO_PKG_VERSION"))?,
            base: BASE.to_string(),
        })
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    /// The underlying HTTP client, for the raw `api` passthrough.
    pub fn http(&self) -> &reqwest::blocking::Client {
        &self.client
    }

    /// One `query` request against `layer`. `params` are appended verbatim.
    fn query_raw(&self, layer: &str, params: &[(&str, &str)]) -> Result<Value, CliError> {
        let url = format!("{}{layer}/query", self.base);
        let resp = self.client.get(&url).query(params).send()?;
        let body = pk_cli_http::json_response(resp)?;
        check_arcgis_error(&body)?;
        Ok(body)
    }

    /// Total rows matching `where_clause`, via `returnCountOnly`.
    ///
    /// Worth the extra request: it turns "we returned 50 matches, there may be
    /// more" into an exact count, so `search` can tell you it is showing 50 of
    /// 837 rather than leaving you guessing.
    pub fn count(&self, layer: &str, where_clause: &str) -> Result<u64, CliError> {
        let body = self.query_raw(
            layer,
            &[
                ("where", where_clause),
                ("returnCountOnly", "true"),
                ("f", "json"),
            ],
        )?;
        body.get("count")
            .and_then(Value::as_u64)
            .ok_or_else(|| CliError::Upstream("ArcGIS count response had no `count`".into()))
    }

    /// Fetch up to `limit` rows matching `where_clause`, paging transparently
    /// when the server caps a response. `order_by` is an ArcGIS
    /// `orderByFields` expression; a stable sort keeps paging coherent.
    pub fn query_all(
        &self,
        layer: &str,
        where_clause: &str,
        out_fields: &str,
        order_by: &str,
        limit: u32,
    ) -> Result<Vec<Value>, CliError> {
        let mut rows: Vec<Value> = Vec::new();
        let mut offset: u32 = 0;

        while rows.len() < limit as usize {
            let want = (limit - rows.len() as u32).min(MAX_RECORD_COUNT);
            let body = self.query_raw(
                layer,
                &[
                    ("where", where_clause),
                    ("outFields", out_fields),
                    ("orderByFields", order_by),
                    ("returnGeometry", "false"),
                    ("resultOffset", &offset.to_string()),
                    ("resultRecordCount", &want.to_string()),
                    ("f", "json"),
                ],
            )?;

            let page: Vec<Value> = body
                .get("features")
                .and_then(Value::as_array)
                .map(|f| {
                    f.iter()
                        .filter_map(|feat| feat.get("attributes").cloned())
                        .collect()
                })
                .unwrap_or_default();

            let got = page.len() as u32;
            rows.extend(page);

            // Stop when the server says there is nothing beyond this page, or
            // when it returned a short page (which means the same thing). Both
            // checks matter: `exceededTransferLimit` is absent on a final page.
            let more = body
                .get("exceededTransferLimit")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if !more || got == 0 || got < want {
                break;
            }
            offset += got;
        }

        rows.truncate(limit as usize);
        Ok(rows)
    }

    /// Every column for one parcel, by PCN.
    pub fn parcel(&self, pcn: &str) -> Result<Option<Value>, CliError> {
        let where_clause = format!("PARID='{}'", sql_escape(pcn));
        let rows = self.query_all(PARCEL_LAYER, &where_clause, "*", "PARID", 1)?;
        Ok(rows.into_iter().next())
    }
}

/// ArcGIS reports query errors with an HTTP **200** and an `error` object, so a
/// successful status alone proves nothing. Map those onto the exit-code
/// contract: a rejected query is the caller's fault (usage, exit 2), anything
/// else is the provider's (upstream, exit 5).
fn check_arcgis_error(body: &Value) -> Result<(), CliError> {
    let Some(err) = body.get("error") else {
        return Ok(());
    };
    let code = err.get("code").and_then(Value::as_i64).unwrap_or(0);
    let message = err
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("unspecified ArcGIS error");
    let detail = err
        .get("details")
        .and_then(Value::as_array)
        .map(|d| {
            d.iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join("; ")
        })
        .filter(|s| !s.is_empty());

    let full = match detail {
        Some(d) => format!("{message} ({d})"),
        None => message.to_string(),
    };
    Err(match code {
        400 => CliError::Usage(format!("ArcGIS rejected the query: {full}")),
        404 => CliError::NotFound(format!("ArcGIS layer not found: {full}")),
        _ => CliError::Upstream(format!("ArcGIS error {code}: {full}")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn clean_response_is_not_an_error() {
        assert!(check_arcgis_error(&json!({"features": []})).is_ok());
    }

    #[test]
    fn http_200_error_body_becomes_a_usage_error() {
        // The signature ArcGIS gotcha: success status, failure body.
        let body = json!({
            "error": {
                "code": 400,
                "message": "Unable to complete operation.",
                "details": ["Unable to perform query."]
            }
        });
        let err = check_arcgis_error(&body).unwrap_err();
        assert_eq!(err.exit_code(), 2);
        assert!(err.to_string().contains("Unable to perform query"));
    }

    #[test]
    fn unknown_error_codes_map_to_upstream() {
        let err =
            check_arcgis_error(&json!({"error": {"code": 500, "message": "boom"}})).unwrap_err();
        assert_eq!(err.exit_code(), 5);
    }

    #[test]
    fn missing_layer_maps_to_not_found() {
        let err =
            check_arcgis_error(&json!({"error": {"code": 404, "message": "gone"}})).unwrap_err();
        assert_eq!(err.exit_code(), 4);
    }

    #[test]
    fn error_without_details_still_reports_the_message() {
        let err = check_arcgis_error(&json!({"error": {"code": 400, "message": "bad where"}}))
            .unwrap_err();
        assert!(err.to_string().contains("bad where"));
    }
}
