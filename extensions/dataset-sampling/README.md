# Verifiable Dataset Sampling

This extension adds two independent trustworthy-sampling proofs for structured
datasets sold through Fair File Marketplace:

1. `flow-graph`: time-aligned raw transfer samples for reconstructing and
   inspecting a local fund-flow graph.
2. `cluster`: entity samples that disclose the source transfers supporting a
   sampled wallet cluster under a published clustering rule.

Each proof owns its guest, host tooling, fixtures, certificate fields, and UI
integration. Extension-specific logic stays outside the core `drop-lib` crate.

The canonical dataset/sample envelope is defined by [`schema.json`](schema.json).
The flow-graph and cluster directories refine that envelope with their respective
challenge-selection fields and sampled payload requirements.
