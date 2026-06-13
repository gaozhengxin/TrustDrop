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
```

The manifest tracks the current local Foundry broadcast deployment:

- ExchangeHub: `0x2e506eF3F3cE222F276ddA64Df239CEF92683a78`
- Start block: `256170177`

If contracts are redeployed, update `subgraph.yaml` before codegen/build/deploy.
