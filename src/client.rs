//! Thin, polite client over the Palm Beach County enterprise **ArcGIS** server
//! that hosts the Property Appraiser's parcel data.
//!
//! Everything here is anonymous public records: no API key, no token, no login,
//! and — unlike the district portals the sibling CLIs talk to — no WAF, so a
//! plain client User-Agent is fine.
//!
//! The query protocol is `pk-cli-arcgis`'s: errors that arrive as HTTP 200, paging
//! past a short page, and a changed response failing rather than reading as "no
//! parcels here". The layer, its columns and the `where` clauses stay here.

use pk_cli_arcgis::{Layer, Query};
use pk_cli_core::{block_on, CliError};
use pk_cli_http::BlockingTransport;
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
    transport: BlockingTransport,
    base: String,
}

impl Arcgis {
    pub fn new() -> Result<Self, CliError> {
        Ok(Self {
            client: pk_cli_http::client("pbca", env!("CARGO_PKG_VERSION"))?,
            transport: BlockingTransport::new()?,
            base: BASE.to_string(),
        })
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    /// The underlying HTTP client, for the raw `api` passthrough and the detail page.
    pub fn http(&self) -> &reqwest::blocking::Client {
        &self.client
    }

    fn layer(&self, layer: &str) -> Layer<BlockingTransport> {
        Layer::new(self.transport.clone(), format!("{}{layer}", self.base))
            .page_size(MAX_RECORD_COUNT)
            .user_agent("pbca", env!("CARGO_PKG_VERSION"))
    }

    /// Total rows matching `where_clause`, via `returnCountOnly`.
    ///
    /// Worth the extra request: it turns "we returned 50 matches, there may be
    /// more" into an exact count, so `search` can tell you it is showing 50 of
    /// 837 rather than leaving you guessing.
    pub fn count(&self, layer: &str, where_clause: &str) -> Result<u64, CliError> {
        block_on(self.layer(layer).count(where_clause))
    }

    /// Fetch up to `limit` rows matching `where_clause`, paging transparently
    /// when the server caps a response. `order_by` is a list of columns that
    /// identifies a row, which keeps paging coherent.
    pub fn query_all(
        &self,
        layer: &str,
        where_clause: &str,
        out_fields: &str,
        order_by: &str,
        limit: u32,
    ) -> Result<Vec<Value>, CliError> {
        let query = Query::new(where_clause, order_by)
            .fields(out_fields)
            .limit(limit);
        block_on(self.layer(layer).rows(&query))
    }

    /// Every column for one parcel, by PCN.
    pub fn parcel(&self, pcn: &str) -> Result<Option<Value>, CliError> {
        let where_clause = format!("PARID='{}'", sql_escape(pcn));
        let rows = self.query_all(PARCEL_LAYER, &where_clause, "*", "PARID", 1)?;
        Ok(rows.into_iter().next())
    }
}
