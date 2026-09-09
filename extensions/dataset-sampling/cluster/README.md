# Cluster Sampling

This proof binds a challenge-selected entity cluster and its supporting source
transfers to the committed dataset and the published clustering rule.

[`sample.schema.json`](sample.schema.json) requires one or more selected
wallet-cluster IDs and a non-empty array of their source-transfer subgraphs. Its
base dataset shape is [`../dataset.schema.json`](../dataset.schema.json).

The Rust library in [`lib`](lib) deterministically chooses unique indices in the
canonical `wallet-clusters.ndjson` record order. The IDs at those indices become
the certificate's `wallet_cluster_ids`; the library is allocation-free and
`no_std` for future guest reuse.
