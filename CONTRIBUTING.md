# Contributing

Thanks for taking a look. This is a small, focused tool — a client over public
Palm Beach County property records — so the bar is "does it stay simple, honest,
and polite to the county's servers".

## Getting set up

```sh
git clone https://github.com/piekstra/palm-beach-county-appraiser-fl-cli
cd palm-beach-county-appraiser-fl-cli
git config core.hooksPath .githooks     # enables the pre-commit gitleaks + fmt guard
make check                              # fmt + clippy + tests
```

You need a recent stable Rust toolchain. Nothing else — there's no account to
create, no API key to obtain, and no fixtures to download.

## Before you open a PR

```sh
make check      # cargo fmt --check, clippy -D warnings, cargo test
```

CI runs the same three, plus `cargo audit`, `cargo deny`, and gitleaks.

## House rules

- **No PII or real parcel data in the repo.** Tests use synthetic parcels
  (`12345678901234567`, `DOE JANE`). This is the family's hard rule and the
  pre-commit hook enforces the secret half of it. If you need a fixture, invent
  one — never paste a captured response.
- **Don't fork shared behavior.** Errors, exit codes, output rendering, config,
  the HTTP client, and self-update come from the `pk-cli-*` crates in
  [cli-common](https://github.com/piekstra/cli-common). Anything reusable across
  the family belongs there, not here. Surface changes (a new standard command,
  flag, DTO field, or exit code) are spec changes — land them in cli-common's
  `DESIGN.md` first.
- **Read [`docs/pbcpao-api.md`](docs/pbcpao-api.md) before touching
  `src/client.rs` or `src/details.rs`.** It documents the endpoints and the
  non-obvious traps (ArcGIS returning errors as HTTP 200, `PADDR3` not being an
  address line, `PARCEL_SALES_PAPA` not being a sale-history layer). Those cost
  real debugging cycles to find.
- **Parsing is best-effort.** The detail page is a scrape. A missing field
  should become `None`, not a failed command; a changed page should produce a
  clear exit-5 message, never a panic. When a field moves, fix the path *and add
  a test*.
- **Be polite to the county.** No aggressive looping or concurrent hammering.
  Bulk work belongs on the FeatureServer's export formats or the county's
  open-data portal, not on repeated `query` calls.
- **Keep it credential-free.** This data is public by law. If a change seems to
  need a login or the keychain, that's a strong signal it's the wrong change.

## Tests

Unit tests live beside the code in `#[cfg(test)] mod tests`. The ones worth
imitating cover the gotchas rather than the happy path — see
`client.rs::http_200_error_body_becomes_a_usage_error` and
`details.rs::extracts_the_embedded_model_past_braces_in_strings`.

New parsing logic should come with a test built from a synthetic payload shaped
like the real one. Nothing in the suite touches the network.

## Reporting problems

Bugs and feature ideas: open an issue. Security issues: see
[SECURITY.md](SECURITY.md) — please don't open a public issue for those.
