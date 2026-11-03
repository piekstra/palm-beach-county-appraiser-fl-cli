//! `pbca mcp`: the county's public parcel records as MCP tools (cli-common DESIGN.md
//! §1.9, the MCP profile), for Claude Code, Claude Desktop and any MCP client.
//!
//! Four read-only tools over the client and parsers the commands use: `pbca_search`,
//! `pbca_parcel`, `pbca_sales` and `pbca_taxes`. Everything they read is anonymous public
//! record, so there is no credential here and nothing can change anything at the county.
//! The one secret this binary can touch is the bearer token of `mcp --http`, held by
//! `pk-cli-mcp`.
//!
//! A concise result leaves out the owner's mailing address, which is often somewhere other
//! than the parcel, and an exemption's applicant. `detailed` adds them. They are the
//! county's published record either way; the point is that an answer about a house's value
//! does not carry them by default.
//!
//! The config is read on each call, for the default municipality, and listing the tools
//! reads nothing.

use pk_cli_core::{CliError, Money};
use pk_cli_mcp::{
    parse_args, ListArgs, Page, PageRequest, ResponseFormat, Server, ToolDef, ToolError,
    ToolOutput, Toolset,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::client::{Arcgis, PARCEL_LAYER, SEARCH_FIELDS};
use crate::commands::search::build_query;
use crate::details::{self, Details, Exemption, TaxYear};
use crate::model::{Parcel, ParcelSummary, Sale};
use crate::pcn::Pcn;
use crate::util::normalize_query;

pub const SEARCH_TOOL: &str = "pbca_search";
pub const PARCEL_TOOL: &str = "pbca_parcel";
pub const SALES_TOOL: &str = "pbca_sales";
pub const TAXES_TOOL: &str = "pbca_taxes";

/// How many candidates an ambiguous address names.
const CANDIDATES: u32 = 5;
/// The furthest one search reads into its matches. The county is asked for every row up to
/// the page, so a deep page is a bulk read, which belongs on its export formats.
const MAX_SEARCH_ROWS: usize = 1_000;

/// What a tool needs from the county and from this computer. [`Live`] is the real one;
/// the tests answer from scrubbed fixtures.
pub trait Source {
    /// The `municipality` setting, read on each call.
    fn municipality_setting(&self) -> Result<Option<String>, ToolError>;
    fn count(&self, where_clause: &str) -> Result<u64, CliError>;
    fn rows(&self, where_clause: &str, order_by: &str, limit: u32) -> Result<Vec<Value>, CliError>;
    fn parcel(&self, pcn: &Pcn) -> Result<Option<Value>, CliError>;
    fn details(&self, pcn: &Pcn) -> Result<Details, CliError>;
}

/// The county's servers, and the config file.
pub struct Live;

impl Source for Live {
    fn municipality_setting(&self) -> Result<Option<String>, ToolError> {
        crate::config::load()
            .map(|cfg| cfg.municipality)
            .map_err(|_| {
                ToolError::other("the pbca config file cannot be read").with_fix(
                    "Ask the user to run `pbca config show` in a terminal and repair it, then \
                     call the tool again",
                )
            })
    }

    fn count(&self, where_clause: &str) -> Result<u64, CliError> {
        Arcgis::new()?.count(PARCEL_LAYER, where_clause)
    }

    fn rows(&self, where_clause: &str, order_by: &str, limit: u32) -> Result<Vec<Value>, CliError> {
        Arcgis::new()?.query_all(PARCEL_LAYER, where_clause, SEARCH_FIELDS, order_by, limit)
    }

    fn parcel(&self, pcn: &Pcn) -> Result<Option<Value>, CliError> {
        Arcgis::new()?.parcel(pcn.bare())
    }

    fn details(&self, pcn: &Pcn) -> Result<Details, CliError> {
        details::fetch(Arcgis::new()?.http(), pcn)
    }
}

fn amount(m: Option<Money>) -> Option<String> {
    m.map(|m| m.amount)
}

/// A `query` ready for `build_query`, or a usage error. A wildcard would ask the county for
/// a scan of every row, and a prefix match is all the command offers.
fn checked_query(raw: &str) -> Result<String, ToolError> {
    let query = normalize_query(raw);
    if query.is_empty() {
        return Err(ToolError::usage("query must not be empty"));
    }
    if query.contains(['%', '_']) {
        return Err(ToolError::usage(
            "query is matched from its start, so % and _ have no use here; leave them out",
        ));
    }
    Ok(query)
}

/// Which parcel `parcel` means: a parcel control number, or a street address that singles
/// one out.
fn resolve(source: &impl Source, parcel: &str) -> Result<Pcn, ToolError> {
    let raw = parcel.trim();
    if raw.is_empty() {
        return Err(ToolError::usage(
            "parcel must be a parcel control number or a street address",
        ));
    }
    if !raw.chars().any(|c| c.is_ascii_alphabetic()) {
        return Pcn::parse(raw).map_err(ToolError::from);
    }
    let address = checked_query(raw)?;
    let (where_clause, order_by) = build_query(&address, false, None);
    let rows = source.rows(&where_clause, order_by, CANDIDATES + 1)?;
    let exact: Vec<&Value> = rows
        .iter()
        .filter(|r| crate::model::text(r, "SITE_ADDR_STR").as_deref() == Some(address.as_str()))
        .collect();
    let chosen = match (rows.len(), exact.len()) {
        (0, _) => {
            return Err(
                ToolError::not_found(format!("no parcel's address starts with {raw:?}")).with_fix(
                    format!("Call {SEARCH_TOOL} with part of the address or with the owner's name"),
                ),
            )
        }
        (1, _) => rows.first(),
        (_, 1) => exact.first().copied(),
        _ => {
            let shown: Vec<String> = rows
                .iter()
                .take(CANDIDATES as usize)
                .filter_map(ParcelSummary::from_attrs)
                .map(|p| {
                    format!(
                        "{} in {} ({})",
                        p.situs_address.unwrap_or_else(|| "no address".into()),
                        p.municipality.unwrap_or_else(|| "no municipality".into()),
                        p.pcn
                    )
                })
                .collect();
            return Err(ToolError::not_found(format!(
                "{raw:?} matches more than one parcel: {}",
                shown.join(", ")
            ))
            .with_fix(format!(
                "Pass one of those control numbers, or narrow it with {SEARCH_TOOL}"
            )));
        }
    };
    let id = chosen
        .and_then(|r| crate::model::text(r, "PARID"))
        .ok_or_else(|| ToolError::upstream("the parcel layer returned a row with no parcel id"))?;
    Pcn::parse(&id).map_err(|_| ToolError::upstream("the parcel layer returned a bad parcel id"))
}

// ---- pbca_search ------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchArgs {
    /// The start of a street address; with owner, of a name, surname first; or a control number.
    pub query: String,
    /// Match owner names, not addresses.
    #[serde(default)]
    pub owner: bool,
    /// Only this municipality, such as JUPITER. Default: the user's setting.
    #[serde(default)]
    pub municipality: Option<String>,
    #[serde(flatten)]
    pub list: ListArgs,
}

#[derive(Serialize)]
struct SearchFilter<'a> {
    query: &'a str,
    owner: bool,
    municipality: Option<&'a str>,
}

#[derive(Debug, Default, Serialize, JsonSchema)]
pub struct SearchItem {
    pub pcn: String,
    pub owner: Option<String>,
    pub address: Option<String>,
    pub municipality: Option<String>,
    pub market_value: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub confidential: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assessed_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub property_use: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_sale_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_sale_price: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SearchOut {
    /// Parcels matching across every page.
    pub total: u64,
    /// The filter applied.
    pub municipality: Option<String>,
    #[serde(flatten)]
    pub page: Page<SearchItem>,
}

fn search_item(p: ParcelSummary, detailed: bool) -> SearchItem {
    let sale = p.last_sale.filter(|_| detailed);
    let (last_sale_date, last_sale_price) = match sale {
        Some(s) => (s.date, amount(s.price)),
        None => (None, None),
    };
    SearchItem {
        pcn: p.pcn,
        owner: p.owner,
        address: p.situs_address,
        municipality: p.municipality,
        market_value: amount(p.market_value),
        confidential: p.confidential,
        assessed_value: amount(p.assessed_value).filter(|_| detailed),
        property_use: p.property_use.filter(|_| detailed),
        last_sale_date,
        last_sale_price,
    }
}

// ---- pbca_parcel ------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ParcelArgs {
    /// A control number, dashed or bare, or a street address that singles one out.
    pub parcel: String,
    /// Also read year built and square footage, from the county's detail page.
    #[serde(default)]
    pub building: bool,
    /// concise (default) or detailed.
    #[serde(default)]
    pub response_format: Option<ResponseFormat>,
}

#[derive(Debug, Default, Serialize, JsonSchema)]
pub struct ParcelOut {
    pub pcn: String,
    pub owners: Vec<String>,
    pub address: Option<String>,
    pub municipality: Option<String>,
    pub market_value: Option<String>,
    pub assessed_value: Option<String>,
    pub taxable_value: Option<String>,
    pub homestead: Option<bool>,
    pub property_use: Option<String>,
    pub acres: Option<f64>,
    pub last_sale_date: Option<String>,
    pub last_sale_price: Option<String>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub confidential: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year_built: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub living_area_sqft: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_sqft: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub land_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub improvement_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mailing_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subdivision: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub legal_description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coordinates: Option<String>,
}

fn parcel_out(p: Parcel, detailed: bool) -> ParcelOut {
    let sale = p.last_sale.unwrap_or_default();
    let building = p.building.unwrap_or_default();
    let mailing = (!p.mailing_address.is_empty()).then(|| p.mailing_address.one_line());
    ParcelOut {
        pcn: p.pcn,
        owners: p.owners,
        address: p.situs_address,
        municipality: p.municipality,
        market_value: amount(p.values.market_total),
        assessed_value: amount(p.values.assessed),
        taxable_value: amount(p.values.taxable),
        homestead: p.values.homestead,
        property_use: p.property_use,
        acres: p.acres,
        last_sale_date: sale.date,
        last_sale_price: amount(sale.price),
        confidential: p.confidential,
        year_built: building.year_built,
        living_area_sqft: building.living_area_sqft,
        total_sqft: building.total_sqft,
        land_value: amount(p.values.market_land).filter(|_| detailed),
        improvement_value: amount(p.values.market_improvement).filter(|_| detailed),
        mailing_address: mailing.filter(|_| detailed),
        subdivision: p.subdivision.filter(|_| detailed),
        legal_description: p.legal_description.filter(|_| detailed),
        coordinates: p.coordinates.filter(|_| detailed),
    }
}

// ---- pbca_sales -------------------------------------------------------------------

#[derive(Debug, Deserialize, JsonSchema)]
pub struct HistoryArgs {
    /// A control number, dashed or bare, or a street address that singles one out.
    pub parcel: String,
    #[serde(flatten)]
    pub list: ListArgs,
}

#[derive(Serialize)]
struct HistoryFilter<'a> {
    parcel: &'a str,
}

#[derive(Debug, Default, Serialize, JsonSchema)]
pub struct SaleItem {
    pub date: Option<String>,
    pub price: Option<String>,
    pub buyer: Option<String>,
    pub instrument: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub book: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SalesOut {
    pub pcn: String,
    #[serde(flatten)]
    pub page: Page<SaleItem>,
}

fn sale_item(s: Sale, detailed: bool) -> SaleItem {
    SaleItem {
        date: s.date,
        price: amount(s.price),
        buyer: s.owner,
        instrument: s.instrument,
        book: s.book.filter(|_| detailed),
        page: s.page.filter(|_| detailed),
    }
}

// ---- pbca_taxes -------------------------------------------------------------------

#[derive(Debug, Default, Serialize, JsonSchema)]
pub struct TaxItem {
    pub tax_year: String,
    pub market_value: Option<String>,
    pub assessed_value: Option<String>,
    pub taxable_value: Option<String>,
    pub total_tax: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub land_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub improvement_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exemption_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ad_valorem_tax: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub non_ad_valorem_tax: Option<String>,
}

#[derive(Debug, Default, Serialize, JsonSchema)]
pub struct ExemptionItem {
    pub description: String,
    pub tax_year: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub applicant: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub soh_base_year: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct TaxesOut {
    pub pcn: String,
    pub exemptions: Vec<ExemptionItem>,
    #[serde(flatten)]
    pub page: Page<TaxItem>,
}

fn tax_item(t: TaxYear, detailed: bool) -> TaxItem {
    TaxItem {
        tax_year: t.tax_year,
        market_value: amount(t.market_total),
        assessed_value: amount(t.assessed),
        taxable_value: amount(t.taxable),
        total_tax: amount(t.total_tax),
        land_value: amount(t.market_land).filter(|_| detailed),
        improvement_value: amount(t.market_improvement).filter(|_| detailed),
        exemption_value: amount(t.exemption).filter(|_| detailed),
        ad_valorem_tax: amount(t.ad_valorem_tax).filter(|_| detailed),
        non_ad_valorem_tax: amount(t.non_ad_valorem_tax).filter(|_| detailed),
    }
}

fn exemption_item(e: Exemption, detailed: bool) -> ExemptionItem {
    ExemptionItem {
        description: e.description,
        tax_year: e.tax_year,
        applicant: e.applicant.filter(|_| detailed),
        soh_base_year: e.soh_base_year.filter(|_| detailed),
    }
}

// ---- the toolset ------------------------------------------------------------------

/// The toolset `pbca mcp` serves.
pub struct PbcaTools<S: Source> {
    source: S,
}

impl<S: Source> PbcaTools<S> {
    pub fn new(source: S) -> Self {
        Self { source }
    }

    fn search(&self, args: Value) -> Result<ToolOutput, ToolError> {
        let args: SearchArgs = parse_args(args)?;
        let query = checked_query(&args.query)?;
        let asked = args
            .municipality
            .as_deref()
            .map(normalize_query)
            .filter(|m| !m.is_empty());
        let municipality = match asked {
            Some(m) => Some(m),
            None => self.source.municipality_setting()?,
        };
        let page = PageRequest::new(
            &args.list,
            &SearchFilter {
                query: &query,
                owner: args.owner,
                municipality: municipality.as_deref(),
            },
        )?;
        if page.wanted() > MAX_SEARCH_ROWS {
            return Err(ToolError::usage(format!(
                "a search reads at most its first {MAX_SEARCH_ROWS} matches"
            ))
            .with_fix("Narrow with municipality or a longer query"));
        }
        let (where_clause, order_by) = build_query(&query, args.owner, municipality.as_deref());
        let total = self.source.count(&where_clause)?;
        let rows = if total == 0 {
            Vec::new()
        } else {
            let wanted = u32::try_from(page.wanted()).unwrap_or(u32::MAX);
            self.source.rows(&where_clause, order_by, wanted)?
        };
        let detailed = args.list.format().is_detailed();
        let items: Vec<SearchItem> = rows
            .iter()
            .filter_map(ParcelSummary::from_attrs)
            .map(|p| search_item(p, detailed))
            .collect();
        ToolOutput::new(&SearchOut {
            total,
            municipality,
            page: page.page(items, "municipality or a longer query")?,
        })
    }

    /// One parcel's detail page. The page answers an unknown parcel with a 500, so a failure
    /// asks the layer which it was.
    fn details(&self, pcn: &Pcn) -> Result<Details, ToolError> {
        self.source.details(pcn).map_err(|e| match e {
            CliError::Upstream(_) if matches!(self.source.parcel(pcn), Ok(None)) => {
                ToolError::not_found(format!("no parcel has the control number {pcn}"))
            }
            other => other.into(),
        })
    }

    fn parcel(&self, args: Value) -> Result<ToolOutput, ToolError> {
        let args: ParcelArgs = parse_args(args)?;
        let pcn = resolve(&self.source, &args.parcel)?;
        let attrs = self.source.parcel(&pcn)?.ok_or_else(|| {
            ToolError::not_found(format!("no parcel has the control number {pcn}"))
        })?;
        let mut parcel = Parcel::from_attrs(&attrs).ok_or_else(|| {
            ToolError::upstream("the parcel layer returned a row with no parcel id")
        })?;
        if args.building {
            let found = self.details(&pcn)?;
            found.require_building()?;
            parcel.building = found.building;
        }
        let detailed = args.response_format.unwrap_or_default().is_detailed();
        ToolOutput::new(&parcel_out(parcel, detailed))
    }

    fn sales(&self, args: Value) -> Result<ToolOutput, ToolError> {
        let args: HistoryArgs = parse_args(args)?;
        let page = PageRequest::new(
            &args.list,
            &HistoryFilter {
                parcel: args.parcel.trim(),
            },
        )?;
        let pcn = resolve(&self.source, &args.parcel)?;
        let detailed = args.list.format().is_detailed();
        let found = self.details(&pcn)?;
        found.require_sales()?;
        let mut sales = found.sales;
        // ISO dates sort as text, and an undated sale sorts last.
        sales.sort_by(|a, b| b.date.cmp(&a.date));
        let items: Vec<SaleItem> = sales.into_iter().map(|s| sale_item(s, detailed)).collect();
        ToolOutput::new(&SalesOut {
            pcn: pcn.dashed(),
            page: page.page(items, "limit")?,
        })
    }

    fn taxes(&self, args: Value) -> Result<ToolOutput, ToolError> {
        let args: HistoryArgs = parse_args(args)?;
        let page = PageRequest::new(
            &args.list,
            &HistoryFilter {
                parcel: args.parcel.trim(),
            },
        )?;
        let pcn = resolve(&self.source, &args.parcel)?;
        let detailed = args.list.format().is_detailed();
        let found = self.details(&pcn)?;
        found.require_rolls()?;
        let items: Vec<TaxItem> = found
            .tax_years
            .into_iter()
            .map(|t| tax_item(t, detailed))
            .collect();
        ToolOutput::new(&TaxesOut {
            pcn: pcn.dashed(),
            exemptions: found
                .exemptions
                .into_iter()
                .map(|e| exemption_item(e, detailed))
                .collect(),
            page: page.page(items, "limit")?,
        })
    }
}

impl<S: Source> Toolset for PbcaTools<S> {
    fn server(&self) -> Server {
        Server::new(
            "pbca",
            env!("CARGO_PKG_VERSION"),
            "Palm Beach County Property Appraiser",
            "pbca",
        )
        // No credential is involved, so "ask the user to sign in" would send them to a
        // command that changes nothing here.
        .hosted(
            "through the open-source `pbca` command-line tool, from the county's anonymous \
             public records with no credentials",
            "tell the user, since nothing here signs in and the county's site may have changed",
        )
        .instructions(
            "These are public records about any parcel in Palm Beach County, Florida, not only \
             the user's. Owner names are real people's, so use them for what the user asked \
             about. Amounts are USD decimal strings. Owner names are stored surname-first. A \
             confidential true means Florida law shields that owner. \
             Unofficial; not affiliated with the Property Appraiser or the county.",
        )
    }

    fn tools(&self) -> Vec<ToolDef> {
        vec![
            ToolDef::read(
                SEARCH_TOOL,
                "Find Palm Beach County parcels",
                "Finds parcels by the start of an address or an owner's name. Use it for a control \
                 number, or for what a street or owner holds. Returns total and items of pcn, \
                 owner, address, municipality and market_value; detailed adds assessed_value, \
                 property_use and the last sale. Narrow with municipality or a longer query.",
            )
            .input::<SearchArgs>()
            .output::<SearchOut>(),
            ToolDef::read(
                PARCEL_TOOL,
                "One parcel's record",
                "One parcel's current record. Use it for who owns a parcel and what it is worth. \
                 Returns owners, values, homestead, use, acres and last sale; building adds year \
                 built and square footage; detailed adds land and improvement values, mailing \
                 address, subdivision, legal description and coordinates.",
            )
            .input::<ParcelArgs>()
            .output::<ParcelOut>(),
            ToolDef::read(
                SALES_TOOL,
                "A parcel's sale history",
                "Every recorded sale of one parcel, newest first. Use it for what a property sold \
                 for and when. Returns date, price, buyer and instrument; detailed adds book and \
                 page.",
            )
            .input::<HistoryArgs>()
            .output::<SalesOut>(),
            ToolDef::read(
                TAXES_TOOL,
                "A parcel's values and taxes by year",
                "One parcel's values and total tax by year, newest first, about ten years, and its \
                 exemptions. Use it for what a property was taxed and how its value moved. Its detailed form \
                 adds land and improvement values, the exemption amount, the two parts of the tax, \
                 and each exemption's applicant and Save Our Homes base year.",
            )
            .input::<HistoryArgs>()
            .output::<TaxesOut>(),
        ]
    }

    async fn call(&self, name: &str, args: Value) -> Result<ToolOutput, ToolError> {
        match name {
            SEARCH_TOOL => self.search(args),
            PARCEL_TOOL => self.parcel(args),
            SALES_TOOL => self.sales(args),
            TAXES_TOOL => self.taxes(args),
            other => Err(ToolError::usage(format!("no tool named {other}"))),
        }
    }
}

#[cfg(test)]
mod tests;
