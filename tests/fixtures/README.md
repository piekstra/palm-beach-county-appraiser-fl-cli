# Test fixtures

Payload shapes for the `src/mcp.rs` tests. Every value is invented, so no parcel, person,
address, date or figure here is real. `src/mcp/tests.rs` holds each identifying field to a
dummy shape.

| File | Shape of | Handling |
|---|---|---|
| `parcel_row.json` | One row's `attributes` from the ArcGIS `PARCEL_INFO` layer | Invented parcel `12345678901234567`, owners `DOE`, address `EXAMPLE ST`, city `TEST CITY`. |
| `detail_model.json` | The `var model` object embedded in the county's detail page | Invented sales, rolls and building, dated 1999 and 2000. |
