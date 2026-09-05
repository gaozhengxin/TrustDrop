# Flow Graph Sampling

This proof binds a challenge-selected, time-aligned transfer sample to the
committed dataset so a buyer can reconstruct and inspect its local flow graph.

[`schema.json`](schema.json) requires a non-empty `entity_flows` array selected
by an explicit `[bucket_start, bucket_end)` time range.
