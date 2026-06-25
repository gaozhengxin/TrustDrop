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

- ExchangeHub: `0x1C01E8E981909926Ed67B5eEfAbfDfeCAcC882a1`
- Start block: `280261185`

If contracts are redeployed, update `subgraph.yaml` before codegen/build/deploy.
