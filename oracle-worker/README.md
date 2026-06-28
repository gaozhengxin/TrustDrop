# TrustDrop Oracle Worker

Centralized prototype oracle for TrustDrop. It reads an Arbitrum Sepolia transaction receipt, verifies the `OracleRequested` log from the configured `OracleProxy`, checks Walrus availability through Blockberry, and submits `submitCentralizedReport(bytes)` with the configured relayer key.

## Endpoints

- `GET /health`: liveness only.
- `GET /status`: readonly readiness check. It does not expose balances, nonces, private keys, or API keys.
- `GET /walrus/blob-status?blobId=<walrus_blob_id>`: checks whether a Walrus blob is visible and returns its expiry metadata.
- `GET /walrus/blob-status?cCipher=0x...`: same check using the hex bytes value emitted by TrustDrop oracle requests.
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

## Walrus blob status

Example:

```sh
TOKEN="$(grep '^ORACLE_WORKER_TOKEN=' ../drop-script/.env | tail -1 | cut -d= -f2-)"
curl -sS -H "Authorization: Bearer $TOKEN" \
  "https://trustdrop-oracle-worker.zhengxingao.workers.dev/walrus/blob-status?blobId=<walrus_blob_id>"
```

Response fields:

- `status`: `0` active, `1` unavailable. Unavailable includes expired and not found.
- `found`: true when Blockberry/Walrus returns blob metadata.
- `expired`: true when the blob exists but its Walrus end epoch has passed.
- `endEpoch`: Walrus end epoch when available.
- `endTime` / `expiresAt`: estimated expiry time derived from the Walrus epoch schedule used by the oracle.
