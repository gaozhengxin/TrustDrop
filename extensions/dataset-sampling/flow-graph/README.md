# Flow Graph Sampling

This proof binds a challenge-selected, time-aligned transfer sample to the
committed dataset so a buyer can reconstruct and inspect its local flow graph.

[`schema.json`](schema.json) requires a non-empty `entity_flows` array selected
by an explicit `[bucket_start, bucket_end)` time range.

The Rust library in [`lib`](lib) deterministically maps a challenge seed onto
one canonical time bucket and verifies that disclosed flow rows use that exact
bucket boundary. It is `no_std` and suitable for reuse by a future guest.
