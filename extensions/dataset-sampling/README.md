# Verifiable Dataset Sampling

This extension adds two independent trustworthy-sampling proofs for structured
datasets sold through Fair File Marketplace:

1. `flow-graph`: time-aligned raw transfer samples for reconstructing and
   inspecting a local fund-flow graph.
2. `cluster`: entity samples that disclose the source transfers supporting a
   sampled wallet cluster under a published clustering rule.

Each proof owns its guest, host tooling, fixtures, certificate fields, and UI
integration. Extension-specific logic stays outside the core `drop-lib` crate.

The complete asset is defined by
[`dataset.schema.json`](dataset.schema.json). It is one composite dataset that
contains both the flow-graph and clustering sections.

The two disclosed sample shapes are defined independently:

- [`flow-graph/sample.schema.json`](flow-graph/sample.schema.json) selects one
  time bucket and carries its flow rows.
- [`cluster/sample.schema.json`](cluster/sample.schema.json) selects one or more
  cluster IDs and carries their complete supporting subgraphs.
