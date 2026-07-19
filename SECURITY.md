# Security policy

## Reporting a vulnerability

Please report security issues **privately** — do not open a public issue.

- Preferred: open a [GitHub private security advisory](https://docs.github.com/en/code-security/security-advisories)
  on this repo ("Security" tab → "Report a vulnerability").
- Or contact the maintainer.

We aim to acknowledge within a few days and coordinate a fix/disclosure with you.

## Notes for this tool

`pbca` handles **no credentials**. The Palm Beach County Property Appraiser's
parcel data is anonymous public records: there is no login, no API key, no
token, and no cookie session. Nothing is written to the OS keychain, and the
only thing written to disk is `~/.config/pbca/config.json`, which holds
non-secret preferences (a default municipality and result limit).

That removes the usual credential-handling risks. What's left:

- **No secrets or PII in the repo.** Tests and fixtures use synthetic parcels;
  the pre-commit hook runs `gitleaks`; `.gitignore` blocks `*.html`, `*.csv`,
  and `*.geojson` so a scraped page or bulk export can't be committed by
  accident.
- **Injection into upstream queries.** Search terms are interpolated into an
  ArcGIS `where` clause, so single quotes are escaped (`util::sql_escape`) and
  the behavior is covered by tests. The layer is read-only and anonymous, so the
  exposure is a malformed query rather than data disclosure — but the escaping
  is there and should stay.
- **Dependency advisories** (`cargo audit` / `cargo deny` in CI).
- **Parsing untrusted responses.** The county detail page is scraped. Parsing is
  total and best-effort: malformed or changed input yields a clean exit-5 error,
  never a panic or unbounded allocation.

## A note on the data itself

This tool reads public records and shows what the county publishes. It is not a
disclosure vector — the same data is served to anyone from the county's own
website. Where the county marks an owner statutorily confidential
(Fla. Stat. §119.071), that status is surfaced as a `confidential` field rather
than silently dropped.

If you believe a *specific record* should not be public, that's a matter for the
Palm Beach County Property Appraiser's office, not this repo — we mirror their
publication decisions and have no ability to change them.

## Supported versions

Pre-1.0: only the latest release receives fixes.
