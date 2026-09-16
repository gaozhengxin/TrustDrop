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

## Verified deployment snapshot

The dataset-sampling guests and their standalone Arbitrum Sepolia verifiers
were checked together on 2026-09-16. Both verifier contracts use the SP1
Groth16 gateway at `0x397A5f7f3dBd538f23DE225B51f532c34448dA9B`.

| Proof | ELF SHA-256 | Program vkey | Verifier |
| --- | --- | --- | --- |
| Flow graph | `ea0522b0d87c5690cc976647b3d1a652630d4a07080e39991dd50c52bb02baf7` | `0x00d47eb96b8c3846c8812056885424a768e4838a2796e98a2bf53fd7efef85ee` | `0xf8D06350C5b261e79ccA1A1061A6bd7922a3b09d` |
| Cluster | `64b29373865334773790c5fad1a6aca63c301a40e55deb1186e75fd753920bf8` | `0x00749a17219133a8c64a776926fe18e7d6499072e27cd24ef182eecfd784434a` | `0xe5Ba837f440AC5460C4cc02E5f48fe04994E68e6` |

Changing either guest changes its program vkey and requires a new proof plus a
new verifier deployment. Host-only tooling and documentation changes do not.
