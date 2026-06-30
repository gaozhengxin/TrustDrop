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

- ExchangeHub: `0xc857542964E8F7618F1A372c36E180D5670b1669`
- Start block: `282682922`

If contracts are redeployed, update `subgraph.yaml` before codegen/build/deploy.
