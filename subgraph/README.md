# TrustDrop Subgraph

This subgraph indexes the current Arbitrum Sepolia TrustDrop deployment.

The Studio project is configured through `subgraph/.env`:

- `SUBGRAPH_SLUG`
- `DEPLOY_KEY`

Common commands:

```sh
pnpm install
pnpm --dir subgraph codegen
pnpm --dir subgraph build
pnpm --dir subgraph deploy:studio
pnpm --dir subgraph check:marketplace
```

The manifest tracks the current local Foundry broadcast deployment:

- ExchangeHub: `0x4845b28ae7e3e558A445a1A03ACD07d7c55976d1`
- Start block: `283665140`
- Current Studio query endpoint: `https://api.studio.thegraph.com/query/1722405/test-arbitrum-store/v0.0.10`

If contracts are redeployed, update `subgraph.yaml` before codegen/build/deploy.

## Marketplace capability checks

`pnpm --dir subgraph check:marketplace` reads `SUBGRAPH_QUERY_URL` from `subgraph/.env` or the shell and checks:

- Sale marketplace fields.
- Tag aggregate entity.
- Channel aggregate counters.
- Basic asset queries.
- Time filtering.
- Purchase and settlement count sorting.
- Purchase / settlement / refund source entities for buyer records.
- Frontend recommendation inputs.

The Graph does not provide general fuzzy text search for this subgraph schema. The first marketplace version should query candidate rows from the subgraph, then run fuzzy matching in the frontend over `title`, `description`, `tags`, and `normalizedTags`.

The current deployment is `v0.0.12`. It starts from ExchangeHub block `283721189`. Buyer VSS key transport is carried by the channel `encryptedVssKey` payload; `PurchaseEvent` does not expose ECIES internals.
