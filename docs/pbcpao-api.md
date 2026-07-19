# The Palm Beach County Property Appraiser's public data

What `pbca` talks to, and the gotchas that shaped the client. Everything here
was verified against live endpoints, but **no real parcel data appears in this
repo**: every address, control number, and figure below is invented and shows
only the *shape* of a response, never a captured record.

There are **two** sources, and they are good at different things.

| | ArcGIS parcel layer | County detail page |
|---|---|---|
| Good for | searching, bulk, exact counts | one parcel, in depth |
| Search by address/owner | ✅ | ❌ (needs a PCN) |
| Owner, mailing, values, exemption total | ✅ | ✅ |
| Sale history | ❌ latest sale only | ✅ all sales |
| Tax roll history | ❌ | ✅ ~10 years |
| Year built / square footage | ❌ | ✅ |
| Stability | schema-stable | a scrape |

`pbca` reads ArcGIS by default and only touches the detail page for what ArcGIS
cannot answer (`sales`, `taxes`, `building`, and `parcel --details`).

## 1. ArcGIS — `PARCEL_INFO` layer 4

```
https://gis.pbcgov.org/arcgis/rest/services/Parcels/PARCEL_INFO/FeatureServer/4/query
```

Anonymous: no key, no token, **no WAF** — a plain `curl` with no browser
User-Agent works. 684,864 features, one row per parcel, full countywide
coverage. `maxRecordCount` is 5000.

Sibling layers live in the same `Parcels` folder and share this schema:
`PARCEL_SALES_PAPA/FeatureServer/12`, `PARCEL_QSALES_PAPA`, `QSALES`,
`HOMESTEAD`, `PARCEL_PROPERTY_USE_PAPA`, and `PARCELS/FeatureServer/0`
(geometry + PCN only).

> **`PARCEL_SALES_PAPA` is not a sale-history table.** Despite the name, it has
> exactly 684,864 features — the same as the parcel layer — i.e. one row per
> *parcel* carrying its latest sale, not one row per *sale*. There is no
> sale-history layer anywhere on the ArcGIS server. Full history comes from the
> detail page (§2). Verify with `returnCountOnly=true` before assuming
> otherwise.

**Avoid** `maps.co.palm-beach.fl.us/.../Parcels/MapServer` — cached tiles, no
attributes. The pbcpao.gov homepage search box is a Swiftype *website* search,
not a parcel search.

### The three search modes

| Mode | `where` clause |
|---|---|
| Address | `SITE_ADDR_STR LIKE '100 EXAMPLE%'` |
| Owner | `OWNER_NAME1 LIKE 'SMITH J%'` (also `OWNER_NAME2`) |
| PCN | `PARID='12345678901234567'` |

`SITE_ADDR_STR` is uppercase, `"<num> <name> <suffix>"`, with no city or unit —
normalize input to uppercase and append `%`. Owner names are **surname-first**.
`PARID` is 17 digits, undashed; the dashed form `12-34-56-78-90-123-4567` is
display-only.

A geocoder exists (`Geocoder_New/Situs_115/GeocodeServer`) but the
`SITE_ADDR_STR LIKE` query is cleaner and is what `pbca` uses.

### Paste-ready

```sh
# Address → owner + values
curl -s 'https://gis.pbcgov.org/arcgis/rest/services/Parcels/PARCEL_INFO/FeatureServer/4/query' \
  --data-urlencode "where=SITE_ADDR_STR LIKE '100 EXAMPLE%'" \
  --data-urlencode "outFields=PARID,OWNER_NAME1,SITE_ADDR_STR,TOTAL_MARKET" \
  --data-urlencode "returnGeometry=false" --data-urlencode "f=json"

# Exact match count — cheap, and how `pbca search` reports "N of M"
curl -s '.../FeatureServer/4/query?where=1%3D1&returnCountOnly=true&f=json'
```

### Field reference (layer 4, 58 columns)

| Purpose | Field(s) | Notes |
|---|---|---|
| PCN | `PARID`, `PARCEL_NUMBER` | 17-digit undashed |
| Owner | `OWNER_NAME1`, `OWNER_NAME2` | surname-first |
| Situs | `SITE_ADDR_STR`, `MUNICIPALITY`; components `STREET_NUMBER`/`PRE_DIR`/`STREET_NAME`/`STREET_SUFFIX_ABBR`/`POST_DIR` | uppercase |
| Mailing | `PADDR1`, `PADDR2`, `PADDR3`, `CITYNAME`, `STATE`, `ZIP1`, `ZIP2` | see gotchas |
| Market | `TOTAL_MARKET`, `LAND_MARKET`, `IMPRV_MRKT` | |
| Assessed/taxable | `ASSESSED_VAL`, `TOTAL_TAXABLE`, `TOTAL_VALUE` | |
| Caps | `MKT_CAPPED`, `MKT_NOT_CAPPED`, `CAP_ADJ_VAL`, `AG_USE_VAL` | |
| Exemptions | `EXEMPTION` (amount), `HMSTD_FLG` (Y/N) | amount only; itemized list is on the detail page |
| Last sale | `SALE_DATE`, `PRICE`, `BOOK`, `PAGE`, `INSTRUMENT`, `QUAL_CODE`, `Q_SALE_DATE` | latest only |
| Use / legal | `PROPERTY_USE`, `SUBDIV_NAME`, `LEGAL1`/`2`/`3` | often null |
| Confidentiality | `CONFID_FLG` | `Y` = statutorily confidential owner |
| Geo | `LATLNG`, `ACRES`, geometry | pass `returnGeometry=false` |

### Gotchas

- **Errors arrive as HTTP 200.** A bad `where` returns
  `{"error":{"code":400,...}}` with a success status. Inspect the body; never
  trust the status alone. (`client.rs::check_arcgis_error`.)
- **`PRICE` is a string**; `SALE_DATE` / `Q_SALE_DATE` are **epoch
  milliseconds**. Convert both on parse.
- **`PADDR3` is not a third street line** — it is a pre-formatted
  `CITY STATE ZIP` that duplicates `CITYNAME`/`STATE`/`ZIP1`. Rendering it as an
  address line prints the city twice.
- **`OWNER_NAME1` may end in `&`**, the county's "and others" marker. Joining it
  to `OWNER_NAME2` verbatim yields `NAME & & NAME`.
- **Paging:** a capped response sets `exceededTransferLimit`; fetch the next page
  with `resultOffset`. `exceededTransferLimit` is *absent* on a final page, so
  also stop on a short page.
- Honor `CONFID_FLG='Y'` if you want owner-privacy parity. Not required — this
  *is* the authoritative public source.
- **Bulk:** the FeatureServer exports `csv`/`geojson`/`shapefile`/`filegdb`/
  `sqlite`, and the county runs an ArcGIS Hub open-data portal plus an annual
  tax-roll download. Use those for bulk — don't hammer `query`.

## 2. The county detail page

```
https://pbcpao.gov/Property/Details?parcelId=<PCN17>
```

Anonymous, no browser User-Agent required. The page is server-rendered HTML, but
it embeds its entire dataset as a single JSON literal in a `<script>` tag:

```js
var model = { propertyDetail, ownerInfo, salesInfo, assessmentInfo,
              appraisalInfo, taxInfo, exemptionInfo, extraDetails,
              landDetails, structuralDetails };
```

Reading that literal is far steadier than scraping the rendered tables — it is
the same object the page's own JavaScript binds to, so it moves only when the
county's data model moves, not when the site is restyled.

```sh
curl -s "https://pbcpao.gov/Property/Details?parcelId=<PCN17>" | grep -o 'var model = {.*};'
```

| Section | Shape | Feeds |
|---|---|---|
| `salesInfo[]` | `SaleDate` (MM/DD/YYYY), `Price`, `Book`, `Page`, `SaleType`, `OwnerName` | `pbca sales` |
| `appraisalInfo[]` | `TaxYear`, `ImprovementValue`, `LandValue`, `TotalMarketValue` | `pbca taxes` |
| `assessmentInfo[]` | `TaxYear`, `AssessedValue`, `ExemptionAmount`, `TaxableValue` | `pbca taxes` |
| `taxInfo[]` | `TaxYear`, `AdValoremTax`, `NonAdValoremTax`, `TotalTaxValue` | `pbca taxes` |
| `exemptionInfo[]` | `Description`, `TaxYear`, `ApplicantName`, `SOHBaseYear` | `pbca taxes` |
| `structuralDetails` | `BuildingNumbers[]`, `StructuralElements[]` of `{ElementName, ElementValue}` | `pbca building` |

The three per-year tables are **parallel and separately keyed by `TaxYear`** —
`details.rs` merges them into one row per year.

### Gotchas

- **"Area Under Air" is the living area.** The page's own "Total Square Feet"
  (and `propertyDetail.SqFt`) is area *under roof* — it includes garage and
  porches — despite a site footnote hinting it may indicate living area. The two
  differ substantially on any house with a garage, so picking the wrong one
  silently overstates living space. `structuralDetails` also breaks the total
  down by sub-area (`BAS` base area, `FGR` finished garage, `FSP` finished
  screened porch, …), and those sub-areas sum to the total — a useful check
  that you've read the right field.
- **Year Built** is a `StructuralElements` row, not a top-level field.
- Parcels can carry **several buildings** (`BuildingNumbers`), each element row
  tagged with its `BuildingNumber`.
- Dates here are `MM/DD/YYYY`, unlike ArcGIS's epoch milliseconds.
- Money values are whole-dollar strings (`"600000"`).
- `/Property/Detail` (singular) 404s. The route is `/Property/Details`.
- A related printable view is `/Property/RenderPrintSum?parcelId=<PCN17>`, but it
  carries only ~5 years of history and no structural details — `Details` is
  strictly richer.
- **This is still a scrape.** Every field is optional and parsing is best-effort;
  a missing field yields `None` rather than failing the command. If the county
  renames something, fix the path and add a test — don't make a field required.
