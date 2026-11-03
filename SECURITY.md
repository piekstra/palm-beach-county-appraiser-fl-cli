# Security policy

## Reporting a vulnerability

Please report security issues **privately** — do not open a public issue.

- Preferred: open a [GitHub private security advisory](https://docs.github.com/en/code-security/security-advisories)
  on this repo ("Security" tab → "Report a vulnerability").
- Or contact the maintainer.

We aim to acknowledge within a few days and coordinate a fix/disclosure with you.

## Notes for this tool

`pbca` holds **no credential for the county**. The Palm Beach County Property
Appraiser's parcel data is anonymous public records: there is no login, no API
key, and no cookie session. The only file it writes is
`${XDG_CONFIG_HOME:-~/.config}/pbca/config.json` (on Windows, `~` is
`%USERPROFILE%`), which holds non-secret preferences (a default municipality and
result limit). The one secret it can hold is the bearer token of `pbca mcp
--http`, below.

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

## MCP tools

`pbca mcp` serves four read-only tools to Claude: `pbca_search`, `pbca_parcel`, `pbca_sales`
and `pbca_taxes`. None writes anything, and none takes a path, a URL or a raw `where` clause:
a search's text is escaped, and `%` and `_` are refused. The config is read on each call. On
stdio nothing touches the keychain, and listing the tools reads no file.

**What reaches Claude.** Tool results go to Claude as part of your conversation, and these
records name real people: every result carries an owner's name, and a detailed result adds a
mailing address or an exemption's applicant. The county publishes all of it, and a person's
parcels can be found by name, as on the county's own site. Text from the county's records is
third-party data; the server's instructions tell the model to treat it as data, never as
instructions.

`pbca mcp --http --port <PORT>` serves the same tools for your own Cloudflare Worker, which
reaches it through your own Cloudflare Tunnel (cli-common's `connector/`). It binds 127.0.0.1
only, with no flag for another address, and refuses to start without a bearer token of 32 or
more characters, from `PBCA_MCP_TOKEN` or the keychain item `mcp-http-token` (`piekstra.pbca`),
which `pbca mcp --new-token` makes from 256 random bits and replaces only with `--overwrite`.
Every request must carry the token (compared in constant time) and a `Host` of `127.0.0.1` or
`localhost`, and must not carry an `Origin`; there are no CORS headers. A copy of that token
lives in your Worker as its `HOME_PBCA_TOKEN` secret. The data is public, so a leaked token
gives no account to anyone: it can read county records from your computer's address.

**Tool results also cross Cloudflare on this path**, through your tunnel and your Worker, on
their way to Claude. Who holds what, and what a leaked token can do, is in the connector's
[threat model](https://github.com/piekstra/cli-common/blob/main/connector/README.md#threat-model).

`pbca` has no `auth logout`, since it has no sign-in. To revoke the connector's access, run
`pbca mcp --new-token --overwrite` and restart the server (`launchctl kickstart -k gui/$(id
-u)/com.piekstra.pbca-mcp-http`); the old token works until the restart, and a token in
`$PBCA_MCP_TOKEN` wins until you change it. To stop serving, run `launchctl bootout gui/$(id
-u)/com.piekstra.pbca-mcp-http` and stop the tunnel. To remove the token on macOS, run
`security delete-generic-password -s piekstra.pbca -a mcp-http-token`.

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
