# HyperCast.Corpus

HyperCast's cross-language conformance corpus — the `corpus/*.json` files at the root of the
[HyperCast repository](https://github.com/SkunkWerkx/HyperCast) — as a content-only NuGet
package, versioned in lockstep with the `HyperCast` package. Every one of HyperCast's
bindings replays these same vectors against the same native core before it ships; this
package lets a downstream suite that routes its own parsers through HyperCast replay them
too, against the exact core its `HyperCast` reference was proven with, instead of vendoring
the files and tracking a snapshot SHA beside them.

```sh
dotnet add package HyperCast.Corpus
```

The files copy to the consumer's output directory under `HyperCastCorpus/`, so a test opens
them relative to `AppContext.BaseDirectory`:

```csharp
var path = Path.Combine(AppContext.BaseDirectory, "HyperCastCorpus", "integer.json");
using var document = JsonDocument.Parse(File.ReadAllText(path));
```

## Vector schema

Every file is a JSON array. Each vector carries `input`, `expect` (`ok`, `empty`,
`malformed`, or `out_of_range`), an optional `fault` span `[offset, length]` in UTF-8 bytes
for failures, and a per-domain value shape for `ok` vectors:

| File | Door | `ok` value shape and declarations |
| --- | --- | --- |
| `boolean.json` | `Cast.Boolean` | `value`: bool |
| `integer.json` | `Cast.SByte` … `Cast.UInt64` | `type` (`i8` … `u64`), optional `format`, `value` |
| `real.json` | `Cast.Single` / `Cast.Double` | `type` (`f32`/`f64`), optional `format`, `value` |
| `decimal.json` | `Cast.Decimal` | optional `format`; `magnitude` (decimal string of the 96-bit magnitude), `scale`, `negative`, and `value` (canonical text, no trailing fraction zeros) |
| `uuid.json` | `Cast.Uuid` | `value`: 32 hex digits, RFC 9562 order |
| `timestamp.json` | `Cast.Timestamp` | `seconds`, `nanos` |
| `unix.json` | `Cast.Unix` | `precision` (1–4), `seconds`, `nanos` |
| `excel_serial.json` | `Cast.ExcelSerial` | `epoch` (1 or 2), `seconds`, `nanos` |
| `date.json` | `Cast.Date` | `year`, `month`, `day` |
| `date_order.json` | `Cast.Date(…, DateOrder)` | `order` (1–3), `year`, `month`, `day` |
| `datetime.json` | `Cast.DateTime` | `order`, `year`, `month`, `day`, `nanos_of_day` |
| `time.json` | `Cast.Time` | `nanos` since midnight |
| `duration.json` | `Cast.Duration` | `seconds`, `nanos` (same sign) |

A `format` object is `{ "decimal_sep", "group_sep", "flags", "currency"? }` — the fields of
a `NumFormat`, with `flags` the raw `NumStyles` bits and `currency` the declared symbol when
one is. Absent, the vector runs under `NumFormat.Invariant`.

The C# binding's own `CorpusTests` in the repository is a complete, current reader for every
file and is the reference for how each value shape maps onto the .NET types the doors return.
