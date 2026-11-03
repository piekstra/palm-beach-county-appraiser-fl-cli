//! The tools over canned answers from the scrubbed fixtures: what each one reads, and what
//! it says. `Live` is a thin call into the client and the parser, tested in their modules.

use std::sync::Mutex;

use pk_cli_mcp::conformance::{assert_conforms, assert_fits, call};
use pk_cli_mcp::{tools_list_json, CallToolResult};
use serde_json::{json, Value};

use super::*;

const PCN: &str = "12345678901234567";
const DASHED: &str = "12-34-56-78-90-123-4567";

fn fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(path).unwrap()
}

fn row() -> Value {
    serde_json::from_str(&fixture("parcel_row.json")).unwrap()
}

/// `row` under another parcel id and address.
fn row_at(id: u64, address: &str) -> Value {
    let mut r = row();
    r["PARID"] = json!(id.to_string());
    r["SITE_ADDR_STR"] = json!(address);
    r
}

fn details() -> Details {
    let page = format!(
        "<script>var model = {};</script>",
        fixture("detail_model.json")
    );
    details::parse(&page).unwrap()
}

/// Answers from the fixtures, and records what the tools asked of it.
#[derive(Default)]
struct Canned {
    setting: Option<String>,
    rows: Vec<Value>,
    missing_parcel: bool,
    failure: Option<CliError>,
    /// What the detail page answers, in place of the fixture.
    page_failure: Option<CliError>,
    page_model: Option<String>,
    asked_where: Mutex<Vec<String>>,
    asked_limit: Mutex<Vec<u32>>,
    asked_parcel: Mutex<Vec<String>>,
    detail_reads: Mutex<u32>,
}

impl Canned {
    fn with_rows(rows: Vec<Value>) -> Self {
        Self {
            rows,
            ..Self::default()
        }
    }

    fn fail(&self) -> Result<(), CliError> {
        self.failure.clone().map_or(Ok(()), Err)
    }
}

impl Source for Canned {
    fn municipality_setting(&self) -> Result<Option<String>, ToolError> {
        Ok(self.setting.clone())
    }

    fn count(&self, where_clause: &str) -> Result<u64, CliError> {
        self.fail()?;
        self.asked_where
            .lock()
            .unwrap()
            .push(where_clause.to_string());
        Ok(self.rows.len() as u64)
    }

    fn rows(&self, where_clause: &str, _: &str, limit: u32) -> Result<Vec<Value>, CliError> {
        self.fail()?;
        self.asked_where
            .lock()
            .unwrap()
            .push(where_clause.to_string());
        self.asked_limit.lock().unwrap().push(limit);
        Ok(self.rows.iter().take(limit as usize).cloned().collect())
    }

    fn parcel(&self, pcn: &Pcn) -> Result<Option<Value>, CliError> {
        self.fail()?;
        self.asked_parcel
            .lock()
            .unwrap()
            .push(pcn.bare().to_string());
        Ok((!self.missing_parcel).then(row))
    }

    fn details(&self, _: &Pcn) -> Result<Details, CliError> {
        self.fail()?;
        *self.detail_reads.lock().unwrap() += 1;
        if let Some(e) = &self.page_failure {
            return Err(e.clone());
        }
        match &self.page_model {
            Some(model) => details::parse(&format!("<script>var model = {model};</script>")),
            None => Ok(details()),
        }
    }
}

fn tools() -> PbcaTools<Canned> {
    PbcaTools::new(Canned::with_rows(vec![row()]))
}

fn fits(tools: &PbcaTools<impl Source>, name: &str, args: Value) -> Value {
    let result = call(tools, name, args);
    assert_fits(tools, name, &result).clone()
}

fn error_text(result: &CallToolResult) -> String {
    assert!(result.is_error(), "expected an error: {}", result.text());
    result.text().to_string()
}

#[test]
fn the_tools_conform_and_every_one_is_read_only() {
    let t = tools();
    assert_conforms(&t);
    let defs = t.tools();
    let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(names, [SEARCH_TOOL, PARCEL_TOOL, SALES_TOOL, TAXES_TOOL]);
    for d in &defs {
        assert!(d.annotations.read_only, "{}", d.name);
        assert!(!d.annotations.destructive, "{}", d.name);
    }
}

/// A source that fails any use: listing the tools must reach none of it.
struct Untouchable;

impl Source for Untouchable {
    fn municipality_setting(&self) -> Result<Option<String>, ToolError> {
        panic!("listing the tools read the config")
    }
    fn count(&self, _: &str) -> Result<u64, CliError> {
        panic!("listing the tools reached the county")
    }
    fn rows(&self, _: &str, _: &str, _: u32) -> Result<Vec<Value>, CliError> {
        panic!("listing the tools reached the county")
    }
    fn parcel(&self, _: &Pcn) -> Result<Option<Value>, CliError> {
        panic!("listing the tools reached the county")
    }
    fn details(&self, _: &Pcn) -> Result<Details, CliError> {
        panic!("listing the tools reached the county")
    }
}

#[test]
fn listing_the_tools_reads_nothing() {
    let list = tools_list_json(&PbcaTools::new(Untouchable));
    assert_eq!(list["tools"].as_array().unwrap().len(), 4);
}

#[test]
fn nothing_here_offers_a_sign_in() {
    let text = pk_cli_mcp::instructions(&tools());
    assert!(text.contains("no credentials"), "{text}");
    assert!(text.contains("not affiliated"), "{text}");
    assert!(!text.contains("auth login"), "{text}");
}

#[test]
fn search_concise_is_short_and_detailed_adds_the_values_and_sale() {
    let t = tools();
    let out = fits(&t, SEARCH_TOOL, json!({"query": "100 example"}));
    assert_eq!(out["total"], 1);
    let item = &out["items"][0];
    assert_eq!(item["pcn"], DASHED);
    assert_eq!(item["owner"], "DOE JANE & DOE JOHN");
    assert_eq!(item["address"], "100 EXAMPLE ST");
    assert_eq!(item["market_value"], "500000.00", "a decimal string");
    assert!(item.get("assessed_value").is_none(), "{item}");
    assert!(item.get("last_sale_price").is_none(), "{item}");
    assert!(item.get("confidential").is_none(), "{item}");

    let out = fits(
        &t,
        SEARCH_TOOL,
        json!({"query": "100 example", "response_format": "detailed"}),
    );
    let item = &out["items"][0];
    assert_eq!(item["assessed_value"], "350000.50");
    assert_eq!(item["last_sale_date"], "2000-01-01");
    assert_eq!(item["last_sale_price"], "100000.00");
}

#[test]
fn search_says_when_the_county_marks_an_owner_confidential() {
    let mut r = row();
    r["CONFID_FLG"] = json!("Y");
    let t = PbcaTools::new(Canned::with_rows(vec![r]));
    let out = fits(&t, SEARCH_TOOL, json!({"query": "100"}));
    assert_eq!(out["items"][0]["confidential"], true);
}

#[test]
fn search_escapes_its_text_and_an_argument_beats_the_setting() {
    let t = PbcaTools::new(Canned {
        setting: Some("SAVED TOWN".into()),
        ..Canned::with_rows(vec![row()])
    });
    let out = fits(&t, SEARCH_TOOL, json!({"query": "o'doe j", "owner": true}));
    assert_eq!(out["municipality"], "SAVED TOWN");
    let asked = t.source.asked_where.lock().unwrap().clone();
    assert_eq!(
        asked[0],
        "(OWNER_NAME1 LIKE 'O''DOE J%' OR OWNER_NAME2 LIKE 'O''DOE J%') \
         AND MUNICIPALITY='SAVED TOWN'"
    );

    let out = fits(
        &t,
        SEARCH_TOOL,
        json!({"query": "100", "municipality": "test city"}),
    );
    assert_eq!(out["municipality"], "TEST CITY");
    let asked = t.source.asked_where.lock().unwrap().clone();
    assert!(
        asked.last().unwrap().ends_with("MUNICIPALITY='TEST CITY'"),
        "{asked:?}"
    );
}

#[test]
fn search_finds_a_control_number_exactly() {
    let t = tools();
    fits(&t, SEARCH_TOOL, json!({"query": DASHED}));
    let asked = t.source.asked_where.lock().unwrap().clone();
    assert_eq!(asked[0], format!("PARID='{PCN}'"));
}

/// Nothing is asked of the county for a query that could only scan it.
#[test]
fn search_refuses_an_empty_query_or_a_wildcard() {
    let t = PbcaTools::new(Untouchable);
    for query in ["", "   ", "%", "1_0 EXAMPLE"] {
        let text = error_text(&call(&t, SEARCH_TOOL, json!({"query": query})));
        assert!(text.starts_with("usage:"), "{query:?}: {text}");
    }
}

#[test]
fn search_pages_and_binds_its_cursor_to_its_filters() {
    let rows: Vec<Value> = (0..3u64)
        .map(|i| row_at(12345678901234560 + i, &format!("{i} EXAMPLE ST")))
        .collect();
    let t = PbcaTools::new(Canned::with_rows(rows));
    let first = fits(&t, SEARCH_TOOL, json!({"query": "EX", "limit": 2}));
    assert_eq!(first["total"], 3);
    assert_eq!(first["returned"], 2);
    assert_eq!(first["items"][0]["address"], "0 EXAMPLE ST");
    assert_eq!(t.source.asked_limit.lock().unwrap().last(), Some(&3));
    let cursor = first["next_cursor"].as_str().unwrap().to_string();

    let second = fits(
        &t,
        SEARCH_TOOL,
        json!({"query": "EX", "limit": 2, "cursor": cursor}),
    );
    assert_eq!(second["returned"], 1);
    assert_eq!(second["items"][0]["address"], "2 EXAMPLE ST");
    assert!(second["next_cursor"].is_null(), "{second}");

    let text = error_text(&call(
        &t,
        SEARCH_TOOL,
        json!({"query": "OTHER", "cursor": cursor}),
    ));
    assert!(text.starts_with("usage:"), "{text}");
}

/// A deep page asks the county for every row before it, so it is refused before asking.
#[test]
fn search_refuses_to_read_past_its_ceiling() {
    let rows: Vec<Value> = (0..1_100u64)
        .map(|i| row_at(12345678901234567 - i, "100 EXAMPLE ST"))
        .collect();
    let t = PbcaTools::new(Canned::with_rows(rows));
    let mut cursor = None;
    let mut pages = 0;
    let text = loop {
        let result = call(
            &t,
            SEARCH_TOOL,
            json!({"query": "100", "limit": 100, "cursor": cursor}),
        );
        if result.is_error() {
            break result.text().to_string();
        }
        pages += 1;
        cursor = assert_fits(&t, SEARCH_TOOL, &result)["next_cursor"]
            .as_str()
            .map(str::to_string);
        assert!(cursor.is_some(), "more than the ceiling matches");
    };
    assert!(text.starts_with("usage:"), "{text}");
    assert!(text.contains("municipality"), "{text}");
    assert_eq!(pages, 9, "the page that would read past 1,000 is refused");
    let asked = t.source.asked_limit.lock().unwrap().clone();
    assert!(asked.iter().all(|limit| *limit <= 1_000), "{asked:?}");
}

/// With no match the county is asked only to count.
#[test]
fn search_with_no_match_is_an_empty_list() {
    let t = PbcaTools::new(Canned::default());
    let out = fits(&t, SEARCH_TOOL, json!({"query": "NOWHERE"}));
    assert_eq!(out["total"], 0);
    assert_eq!(out["returned"], 0);
    assert!(t.source.asked_limit.lock().unwrap().is_empty());
}

#[test]
fn parcel_concise_holds_back_the_mailing_address_and_reads_no_page() {
    let t = tools();
    let out = fits(&t, PARCEL_TOOL, json!({"parcel": DASHED}));
    assert_eq!(out["pcn"], DASHED);
    assert_eq!(out["owners"], json!(["DOE JANE &", "DOE JOHN"]));
    assert_eq!(out["market_value"], "500000.00");
    assert_eq!(out["homestead"], true);
    assert_eq!(out["last_sale_date"], "2000-01-01");
    for held_back in [
        "mailing_address",
        "legal_description",
        "coordinates",
        "land_value",
        "year_built",
    ] {
        assert!(out.get(held_back).is_none(), "{held_back}: {out}");
    }
    assert_eq!(*t.source.detail_reads.lock().unwrap(), 0);
    assert_eq!(t.source.asked_parcel.lock().unwrap().clone(), [PCN]);
}

#[test]
fn parcel_detailed_and_building_add_what_they_say() {
    let t = tools();
    let out = fits(
        &t,
        PARCEL_TOOL,
        json!({"parcel": PCN, "response_format": "detailed"}),
    );
    assert_eq!(out["land_value"], "200000.00");
    assert_eq!(out["improvement_value"], "300000.00");
    assert!(out["mailing_address"]
        .as_str()
        .unwrap()
        .contains("PO BOX 1"));
    assert_eq!(out["coordinates"], "0.1,-0.1");
    assert_eq!(*t.source.detail_reads.lock().unwrap(), 0);

    let out = fits(&t, PARCEL_TOOL, json!({"parcel": PCN, "building": true}));
    assert_eq!(out["year_built"], 1990);
    assert_eq!(out["living_area_sqft"], 2000);
    assert_eq!(out["total_sqft"], 2800);
    assert!(out.get("mailing_address").is_none(), "{out}");
    assert_eq!(*t.source.detail_reads.lock().unwrap(), 1);
}

#[test]
fn a_bad_or_unknown_control_number_says_which() {
    let text = error_text(&call(
        &PbcaTools::new(Untouchable),
        PARCEL_TOOL,
        json!({"parcel": "12-34"}),
    ));
    assert!(text.starts_with("usage:"), "{text}");
    assert!(text.contains("17 digits"), "{text}");

    let t = PbcaTools::new(Canned {
        missing_parcel: true,
        ..Canned::default()
    });
    let text = error_text(&call(&t, PARCEL_TOOL, json!({"parcel": PCN})));
    assert!(text.starts_with("not_found:"), "{text}");
    assert!(text.contains(DASHED), "{text}");
}

#[test]
fn one_match_for_an_address_is_that_parcel() {
    let t = PbcaTools::new(Canned::with_rows(vec![row_at(
        12345678901234560,
        "100 EXAMPLE ST",
    )]));
    fits(&t, PARCEL_TOOL, json!({"parcel": "100 example"}));
    assert_eq!(
        t.source.asked_parcel.lock().unwrap().clone(),
        ["12345678901234560"]
    );
}

#[test]
fn an_exact_address_wins_over_longer_ones() {
    let t = PbcaTools::new(Canned::with_rows(vec![
        row_at(12345678901234560, "100 EXAMPLE ST"),
        row_at(12345678901234561, "100 EXAMPLE ST APT 1"),
    ]));
    fits(&t, PARCEL_TOOL, json!({"parcel": "100 example st"}));
    assert_eq!(
        t.source.asked_parcel.lock().unwrap().clone(),
        ["12345678901234560"]
    );
}

#[test]
fn an_ambiguous_address_lists_the_candidates_and_picks_none() {
    let rows: Vec<Value> = (0..7u64)
        .map(|i| row_at(12345678901234560 + i, &format!("100 EXAMPLE ST APT {i}")))
        .collect();
    let t = PbcaTools::new(Canned::with_rows(rows));
    let text = error_text(&call(&t, SALES_TOOL, json!({"parcel": "100 example st"})));
    assert!(text.starts_with("not_found:"), "{text}");
    assert!(text.contains("more than one parcel"), "{text}");
    assert!(
        text.contains("100 EXAMPLE ST APT 0 in TEST CITY (12-34-56-78-90-123-4560)"),
        "{text}"
    );
    assert!(text.contains("APT 4"), "{text}");
    assert!(!text.contains("APT 5"), "only five candidates: {text}");
    assert_eq!(t.source.asked_limit.lock().unwrap().clone(), [6]);
    assert_eq!(*t.source.detail_reads.lock().unwrap(), 0);
}

#[test]
fn an_address_nothing_matches_is_not_found() {
    let t = PbcaTools::new(Canned::default());
    let text = error_text(&call(&t, TAXES_TOOL, json!({"parcel": "nowhere st"})));
    assert!(text.starts_with("not_found:"), "{text}");
    assert!(text.contains(SEARCH_TOOL), "{text}");
}

#[test]
fn sales_run_newest_first_and_detailed_adds_the_record() {
    let t = tools();
    let out = fits(&t, SALES_TOOL, json!({"parcel": PCN}));
    assert_eq!(out["pcn"], DASHED);
    assert_eq!(out["returned"], 2);
    let first = &out["items"][0];
    assert_eq!(first["date"], "2000-01-01");
    assert_eq!(first["price"], "100000.00");
    assert_eq!(first["buyer"], "DOE JANE");
    assert_eq!(first["instrument"], "WARRANTY DEED");
    assert!(first.get("book").is_none(), "{first}");
    assert_eq!(out["items"][1]["date"], "1999-01-02");

    let out = fits(
        &t,
        SALES_TOOL,
        json!({"parcel": PCN, "response_format": "detailed"}),
    );
    assert_eq!(out["items"][0]["book"], "00001");
    assert_eq!(out["items"][0]["page"], "0002");
}

#[test]
fn sales_page_with_a_cursor() {
    let t = tools();
    let first = fits(&t, SALES_TOOL, json!({"parcel": PCN, "limit": 1}));
    assert_eq!(first["items"][0]["date"], "2000-01-01");
    let cursor = first["next_cursor"].as_str().unwrap();
    let second = fits(
        &t,
        SALES_TOOL,
        json!({"parcel": PCN, "limit": 1, "cursor": cursor}),
    );
    assert_eq!(second["items"][0]["date"], "1999-01-02");
    assert!(second["next_cursor"].is_null(), "{second}");
}

#[test]
fn taxes_concise_holds_back_the_applicant_and_detailed_adds_it() {
    let t = tools();
    let out = fits(&t, TAXES_TOOL, json!({"parcel": PCN}));
    let year = &out["items"][0];
    assert_eq!(year["tax_year"], "2000");
    assert_eq!(year["market_value"], "500000.00");
    assert_eq!(year["taxable_value"], "300000.00");
    assert_eq!(year["total_tax"], "6000.00");
    assert!(year.get("ad_valorem_tax").is_none(), "{year}");
    assert!(
        out["items"][1]["total_tax"].is_null(),
        "not invented: {out}"
    );
    let exemption = &out["exemptions"][0];
    assert_eq!(exemption["description"], "HOMESTEAD");
    assert!(exemption.get("applicant").is_none(), "{exemption}");

    let out = fits(
        &t,
        TAXES_TOOL,
        json!({"parcel": PCN, "response_format": "detailed"}),
    );
    assert_eq!(out["items"][0]["ad_valorem_tax"], "5000.00");
    assert_eq!(out["items"][0]["land_value"], "200000.00");
    assert_eq!(out["exemptions"][0]["applicant"], "DOE JANE");
    assert_eq!(out["exemptions"][0]["soh_base_year"], "1999");
}

/// The county answers an unknown control number on its detail page with a 500.
#[test]
fn an_unknown_parcel_on_the_detail_page_is_not_found_and_an_outage_stays_upstream() {
    let down = || CliError::Upstream("the county property site returned HTTP 500".into());
    let t = PbcaTools::new(Canned {
        missing_parcel: true,
        page_failure: Some(down()),
        ..Canned::default()
    });
    for tool in [SALES_TOOL, TAXES_TOOL] {
        let text = error_text(&call(&t, tool, json!({"parcel": PCN})));
        assert!(text.starts_with("not_found:"), "{tool}: {text}");
    }

    let t = PbcaTools::new(Canned {
        page_failure: Some(down()),
        ..Canned::default()
    });
    for tool in [SALES_TOOL, TAXES_TOOL] {
        let text = error_text(&call(&t, tool, json!({"parcel": PCN})));
        assert!(text.starts_with("upstream:"), "{tool}: {text}");
    }
}

/// An empty answer from a renamed section would be a false "no sales".
#[test]
fn a_renamed_section_is_upstream_and_a_present_empty_one_is_data() {
    let renamed = |from: &str, to: &str| {
        PbcaTools::new(Canned {
            page_model: Some(fixture("detail_model.json").replace(from, to)),
            ..Canned::default()
        })
    };
    let text = error_text(&call(
        &renamed("salesInfo", "saleHistory"),
        SALES_TOOL,
        json!({"parcel": PCN}),
    ));
    assert!(text.starts_with("upstream:"), "{text}");
    for section in [
        "appraisalInfo",
        "assessmentInfo",
        "taxInfo",
        "exemptionInfo",
    ] {
        let text = error_text(&call(
            &renamed(section, "renamedSection"),
            TAXES_TOOL,
            json!({"parcel": PCN}),
        ));
        assert!(text.starts_with("upstream:"), "{section}: {text}");
    }
    let text = error_text(&call(
        &renamed("structuralDetails", "structure"),
        PARCEL_TOOL,
        json!({"parcel": PCN, "building": true}),
    ));
    assert!(text.starts_with("upstream:"), "{text}");

    let empty = PbcaTools::new(Canned {
        page_model: Some(
            r#"{"salesInfo": [], "appraisalInfo": [], "assessmentInfo": [], "taxInfo": [],
                "exemptionInfo": [], "structuralDetails": {}}"#
                .into(),
        ),
        ..Canned::default()
    });
    let out = fits(&empty, SALES_TOOL, json!({"parcel": PCN}));
    assert_eq!(out["returned"], 0);
    let out = fits(&empty, TAXES_TOOL, json!({"parcel": PCN}));
    assert_eq!(out["returned"], 0);
}

#[test]
fn a_failure_at_the_county_is_upstream_for_every_tool() {
    let t = PbcaTools::new(Canned {
        failure: Some(CliError::Upstream("the county server is down".into())),
        ..Canned::default()
    });
    for (tool, args) in [
        (SEARCH_TOOL, json!({"query": "100"})),
        (PARCEL_TOOL, json!({"parcel": PCN})),
        (SALES_TOOL, json!({"parcel": PCN})),
        (TAXES_TOOL, json!({"parcel": PCN})),
    ] {
        let text = error_text(&call(&t, tool, args));
        assert!(text.starts_with("upstream:"), "{tool}: {text}");
        assert!(!text.contains("auth login"), "{tool}: {text}");
    }
}

#[test]
fn a_bad_argument_or_tool_name_is_a_usage_error() {
    let t = PbcaTools::new(Untouchable);
    let text = error_text(&call(&t, PARCEL_TOOL, json!({"pcn": PCN})));
    assert!(text.starts_with("usage:"), "{text}");
    let text = pk_cli_core::block_on(t.call("pbca_order", json!({})))
        .unwrap_err()
        .render(&t.server());
    assert!(text.starts_with("usage:"), "{text}");
}

/// Every identifying or dated value in the fixtures holds a dummy's shape.
#[test]
fn the_fixtures_hold_only_dummies() {
    let r = row();
    assert_eq!(r["PARID"], PCN);
    for key in ["OWNER_NAME1", "OWNER_NAME2"] {
        assert!(r[key].as_str().unwrap().starts_with("DOE "), "{key}");
    }
    assert!(r["SITE_ADDR_STR"].as_str().unwrap().contains("EXAMPLE"));
    assert_eq!(r["CITYNAME"], "TEST CITY");
    assert_eq!(r["ZIP1"], "00000");

    let d = details();
    for sale in &d.sales {
        let owner = sale.owner.as_deref().unwrap();
        assert!(
            owner.starts_with("DOE ") || owner.starts_with("ROE "),
            "{owner}"
        );
        let date = sale.date.as_deref().unwrap();
        assert!(
            date.starts_with("1999") || date.starts_with("2000"),
            "{date}"
        );
    }
    for year in &d.tax_years {
        assert!(["1999", "2000"].contains(&year.tax_year.as_str()));
    }
}
