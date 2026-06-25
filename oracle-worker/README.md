# TrustDrop Oracle Worker

Centralized prototype oracle for TrustDrop. It reads an Arbitrum Sepolia transaction receipt, verifies the `OracleRequested` log from the configured `OracleProxy`, checks Walrus availability through Blockberry, and submits `submitCentralizedReport(bytes)` with the configured relayer key.

## Endpoints

- `GET /health`: liveness only.
- `GET /status`: readonly readiness check. It does not expose balances, nonces, private keys, or API keys.
- `POST /oracle/fulfill`: body `{ "chainId": 421614, "txHash": "0x...", "requestLogIndex": 0 }`. `requestLogIndex` is optional unless the transaction emitted multiple matching oracle request logs.

Authenticated endpoints require either:

- `Authorization: Bearer <WORKER_API_TOKEN>`
- `x-worker-token: <WORKER_API_TOKEN>`

## Required secrets

```sh
wrangler secret put ARBITRUM_SEPOLIA_RPC_URL
wrangler secret put ORACLE_RELAYER_PRIVATE_KEY
wrangler secret put BLOCKBERRY_API_KEY
wrangler secret put WORKER_API_TOKEN
```

The relayer address must match `OracleProxy.centralizedOracleSigner()`.
