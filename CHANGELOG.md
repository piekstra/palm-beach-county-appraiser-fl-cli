# Changelog

All notable changes to `pbca` are documented here. Newest first. The project follows
[SemVer](https://semver.org/); until 1.0, minors carry the breaking-change signal.

## 0.2.0 — 2026-11-03

- `pbca mcp`: four read-only tools for Claude: `pbca_search`, `pbca_parcel`, `pbca_sales` and
  `pbca_taxes`. They run over stdio for Claude Code and Claude Desktop, and with `--http` on
  127.0.0.1 behind a bearer token for Claude on the web and mobile through your own
  Cloudflare Worker and tunnel.
- Releases attach `pbca-<version>.mcpb`, a Claude Desktop extension, and a Windows x86_64
  tarball, and publish only once every asset is there.
- `self-update` says to restart a running `pbca mcp --http` and to install the new `.mcpb`.
- The config follows `USERPROFILE` on Windows.
- cli-common v0.26.0, up from v0.15.1. Rust 1.89 or later.
