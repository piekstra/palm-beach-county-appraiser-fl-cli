# palm-beach-county-appraiser-fl

Look up **Palm Beach County, Florida** property records from the command line —
owner, mailing address, parcel control number, market and assessed values,
exemptions, sale history, and building details.

It's a thin, polite client over the county's own public data: the enterprise
**ArcGIS** server that hosts the Property Appraiser's parcel layer, plus the
appraiser's per-parcel detail page for the history that layer doesn't carry.
All of it is anonymous public records — there is **no account, no password, and
no API key** to configure, and nothing is ever written to your keychain.

Built to be **human- and agent-friendly**: every command has a `--json` mode and
stable exit codes, so a person or a script can chain it into a pipeline.

> The repo/crate is `palm-beach-county-appraiser-fl`; the binary is **`pbca`**.

## Install

```sh
cargo build --release        # binary at ./target/release/pbca
# or
cargo install --path .       # installs the `pbca` binary onto your PATH
```

Requires a recent stable Rust toolchain.

## Quick start

```sh
pbca search "main st"              # find parcels by street address
pbca search --owner "smith j"         # ...or by owner name (surname first)
pbca search "main st" -m JUPITER      # narrow to one municipality

pbca parcel   <PCN>                   # full record for one parcel
pbca parcel   <PCN> --details         # ...plus year built + square footage
pbca sales    <PCN>                   # every recorded sale, newest first
pbca taxes    <PCN>                   # ~10 years of values, levies, exemptions
pbca building <PCN>                   # year built, living area, structure
```

A `<PCN>` is a 17-digit parcel control number, and works dashed or bare —
`12-34-56-78-90-123-4567` and `12345678901234567` are the same parcel. `pbca
search` accepts one too, so you can paste any of the three forms into the same
command. Search results print the PCN in the dashed form, ready to feed back in.

### What each source can answer

The county publishes parcel data two ways, and they're good at different things.
`pbca` reads the fast, stable ArcGIS layer by default and only reaches for the
detail page when you ask for something the layer doesn't carry.

| | `search`, `parcel` | `sales`, `taxes`, `building`, `parcel --details` |
|---|---|---|
| Source | ArcGIS parcel layer | county detail page |
| Cost | one request | one extra request |
| Sale history | latest sale only | **all** sales |
| Tax/value history | current roll only | **~10 years** |
| Year built, square footage | not available | **yes** |

See [`docs/pbcpao-api.md`](docs/pbcpao-api.md) for the endpoints, field
reference, and the gotchas behind those rows.

## Saving typing

```sh
pbca config set municipality JUPITER   # default filter for searches
pbca config set limit 50               # default --limit (otherwise 25)
pbca config show
pbca config path
pbca config unset municipality
```

Settings live in `~/.config/pbca/config.json`. None of it is secret — this tool
has no credentials.

## For scripts and agents

Every command takes `--json`, which puts the DTO alone on stdout; diagnostics go
to stderr, so `pbca ... --json | jq` is always safe.

```sh
pbca search "main st" --json | jq -r '.items[].pcn'
pbca parcel <PCN> --json | jq -r '.values.market_total.amount'
pbca taxes  <PCN> --json | jq -r '.items[] | "\(.tax_year) \(.total_tax.amount)"'
```

DTOs are schema-tagged (`parcel/v1`, `parcel-list/v1`, `sale-list/v1`,
`tax-history/v1`, `building/v1`) so you can detect shape changes. Money is always
`{"amount": "123.45", "currency": "USD"}` — a **string** decimal, never a float.
Dates are ISO `YYYY-MM-DD`. Absent fields are omitted rather than emitted as
`null`.

`pbca search --json` reports the true server-side match count, so you can tell
whether you're seeing everything:

```json
{ "schema": "parcel-list/v1", "items": [ ... ], "total": 37, "truncated": true }
```

### Exit codes

| Code | Meaning |
|---|---|
| 0 | success |
| 1 | generic / unexpected error |
| 2 | usage error (bad PCN, unknown config key, empty query) |
| 3 | auth required — *never returned by this tool* |
| 4 | not found (no such parcel) |
| 5 | upstream error (county server down, or its page markup changed) |

### Raw passthrough

The parcel layer publishes ~58 columns and a dozen query options this CLI
doesn't model. Rather than a flag for each, `api` hands the request straight
through:

```sh
pbca api GET '/PARCEL_INFO/FeatureServer/4/query?where=1%3D1&returnCountOnly=true&f=json'
```

Paths resolve against the ArcGIS `Parcels` folder; absolute URLs pass through
untouched.

## Pairs with `lrfl`

The [Loxahatchee River District CLI](https://github.com/piekstra/loxahatchee-river-fl-cli)
finds utility accounts by address, but the district redacts owner names. Feed
its service address into `pbca` and you get the owner, mailing address, values,
exemptions, and sale history:

```sh
lrfl search "MAIN" --json | jq -r '.[].service_address' \
  | while read -r addr; do pbca search "$addr" --json; done
```

The district tells you who owes what; the appraiser tells you who owns it and
what it's worth.

## A note on privacy

This tool shows what the county publishes, and imposes no privacy the county
itself doesn't — the appraiser *is* the authoritative public source for Florida
property ownership. Where the county marks an owner statutorily confidential
(Fla. Stat. §119.071), `pbca` surfaces that as a `confidential` flag so you know
the record is protected. What you do with output you've fetched is your call.

## Development

```sh
make check        # fmt + clippy + tests
make build        # release build
cargo test
```

Conforms to **piekstra-cli/1**, the shared surface spec in
[piekstra/cli-common](https://github.com/piekstra/cli-common). See
[CONTRIBUTING.md](CONTRIBUTING.md) and [AGENTS.md](AGENTS.md).

## License

MIT OR Apache-2.0.

Unofficial. Not affiliated with or endorsed by the Palm Beach County Property
Appraiser or Palm Beach County.
