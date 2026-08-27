# HyperCast
Allocation-free Rust parsers for scalars from untrusted text — booleans, numerics, UUIDs, and temporals. Every parse returns a Verdict: the value, or a reason code with the offending span. Never throws, never allocates. Polyglot by design, with a shared conformance corpus so every binding agrees byte for byte.
