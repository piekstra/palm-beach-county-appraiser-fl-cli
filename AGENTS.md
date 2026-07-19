# AGENTS.md — palm-beach-county-appraiser-fl

Canonical agent entrypoint for this repo. `CLAUDE.md` is a one-line pointer here.

## What this is

A Rust CLI (`pbca`) over the **Palm Beach County, FL Property Appraiser's**
public parcel data. Two sources, deliberately:

1. The county's enterprise **ArcGIS** server (`gis.pbcgov.org`), layer
   `PARCEL_INFO/FeatureServer/4` — 684,864 parcels, one row each. Fast, stable,
   searchable. Powers `search` and `parcel`.
2. The appraiser's **detail page** (`pbcpao.gov/Property/Details`) — a scrape of
   the JSON blob the page embeds. Carries what ArcGIS doesn't: full sale
   history, ~10 years of tax rolls, year built, square footage. Powers `sales`,
   `taxes`, `building`, and `parcel --details`.

Everything is anonymous public records: **no credentials, no keychain, no
`auth login`**. `auth status` exists only because SPEC v1 requires it of every
family member and reports `required: false`.

Structured as a **library + thin binary**: logic lives in the
`palm_beach_county_appraiser_fl` lib (`src/lib.rs`); `main.rs` only parses args
and dispatches into `src/commands/`.

## Build / test / run

```sh
cargo build --release
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --all
make check                      # all of the above

./target/release/pbca search "main st"
./target/release/pbca parcel 12345678901234567 --details --json
```

## Layout

- `src/lib.rs` — library root (declares the modules below).
- `src/main.rs` — thin binary: parse args, build `Ctx`, dispatch to `commands`.
- `src/cli.rs` — clap `Cli` / `Command` definitions. Tested.
- `src/commands/` — one thin module per domain (`search`, `parcel`, `sales`,
  `taxes`, `building`, `api`, `auth`, `config`, `info`, `self_update`,
  `completions`); `mod.rs` holds `Ctx` and the `--limit` resolution order.
- `src/client.rs` — the `Arcgis` client: `where` queries, exact counts,
  transparent paging, and the HTTP-200-error trap. Tested.
- `src/details.rs` — fetches and parses the detail page's embedded `var model`
  blob into sales/tax/exemption/building DTOs. Tested.
- `src/model.rs` — normalized `Parcel`, `ParcelSummary`, `Sale`, `Values`,
  `MailingAddress` built from raw ArcGIS attributes. Tested.
- `src/pcn.rs` — parcel-control-number parsing (dashed ↔ bare 17 digits). Tested.
- `src/formatter.rs` — human vs `--json` rendering.
- `src/util.rs` — epoch-ms dates, SQL escaping, query normalization. Tested.
- `src/config.rs` — non-secret preferences via `pk-cli-config`. Tested.
- `src/update.rs` / `src/version.rs` — self-update wiring and build banner.
- `docs/pbcpao-api.md` — the discovered API: endpoints, fields, gotchas. No PII.
- `.github/workflows/release.yml` — tag `v*` → binaries `self-update` fetches.

## Gotchas & rules

These each cost a real debugging cycle. They're all covered by tests — if you
"fix" one, a test should tell you.

- **ArcGIS returns errors as HTTP 200.** A bad `where` clause comes back
  `{"error":{"code":400,...}}` with a success status. Never trust the status
  alone; go through `check_arcgis_error`.
- **`PARCEL_SALES_PAPA` is not a sale-history layer.** It has exactly the same
  684,864 features as the parcel layer — one row per *parcel*, not per *sale*.
  No sale-history layer exists on the ArcGIS server at all. Full history only
  comes from the detail page. Check `returnCountOnly=true` before believing a
  layer name.
- **`PADDR3` is not a third street line** — it's a pre-formatted
  `CITY STATE ZIP` duplicating `CITYNAME`/`STATE`/`ZIP1`. Using it as an address
  line prints the city twice.
- **`OWNER_NAME1` may end in `&`** (the county's "and others" marker). Naive
  joining renders `NAME & & NAME`. The `owners` array keeps raw values; only the
  display line strips it.
- **`PRICE` is a string; `SALE_DATE` is epoch milliseconds.** The detail page
  instead uses `MM/DD/YYYY`. Both normalize to ISO on the way out.
- **"Area Under Air" is the living area**, not "Total Square Footage" — the
  latter includes garage and porches. The county's own footnote is misleading.
- **Paging:** `exceededTransferLimit` is absent on a final page, so also stop on
  a short page or the loop never ends.
- **Money is never a float.** Doubles from ArcGIS become two-decimal strings
  (`Money`) before they reach any output.
- **The detail page is a scrape.** Every field is optional; parsing is
  best-effort and a missing field yields `None`. If the county renames
  something, fix the path and **add a test** — don't make a field required. A
  markup change must surface as exit 5 with a clear message, never a panic.
- **Be polite.** This is a public records server at human scale. No aggressive
  looping; for bulk use the FeatureServer's export formats or the county's
  open-data portal, not repeated `query` calls.

## Data rules

- **No PII and no real parcel data in the repo.** Not in code, tests, fixtures,
  or commits. Tests use synthetic parcels (`12345678901234567`, `DOE JANE`).
  `.githooks/pre-commit` runs gitleaks; `.gitignore` blocks `*.html`/`*.csv`/
  `*.geojson` so a scraped page can't be committed by accident. A real PCN
  appears only when a user types one at runtime.
- **Mirror the provider — don't impose privacy it doesn't.** This *is* the
  authoritative public source for Florida property ownership; the CLI renders
  what the county serves. Where the county flags an owner statutorily
  confidential (`CONFID_FLG`), that surfaces as a `confidential` field so the
  caller knows. There's no redaction flag — sanitizing shared output is the
  user's call. (The "no PII **in the repo**" rule above is unaffected: it's
  about committed code, never runtime output.)

## The CLI family & cli-common

This CLI conforms to **piekstra-cli/1** — the shared surface spec in
[piekstra/cli-common](https://github.com/piekstra/cli-common) (`DESIGN.md`):
standard `auth` / `config` / `self-update` / `completions` / `info` / `api`
commands, global `--json`, canonical DTOs (`auth-status/v1`, `self-update/v1`,
`cli-info/v1`), and frozen exit codes 0–6.

- **Don't fork shared behavior.** Error/exit-code handling, output rendering,
  config storage, the HTTP client, and self-update come from the `pk-cli-*`
  crates (tag-pinned git deps on cli-common, currently **v0.2.0**). If you need
  a change there — or you're writing anything reusable across the family CLIs
  (fpl, xfin, lrfl, tojfl, …) — add it to cli-common, cut a tag, and bump the
  pin here. Never copy shared code into this repo.
- **Surface changes are spec changes.** A new standard command, flag, DTO field,
  or exit code belongs in cli-common's `DESIGN.md` first; update
  `conformance.md` alongside.
- **No domain profile.** `info` reports `profiles: []`. `utility/v1` describes
  billing accounts and does not apply to property records. Adding a
  `property/v1` profile would need a second CLI in the domain plus a consumer
  paying for the variance today — see cli-common's `PROFILES.md`.
- **No keychain.** Don't add `pk-cli-secrets` usage or an `auth login`. If a
  future feature seems to need a credential, it's probably the wrong feature —
  this data is public by law.
