---
name: drop-script-env-check
description: Use when preparing or diagnosing TrustDrop drop-script full-flow integration on Arbitrum Sepolia with Walrus mainnet, including readonly checks for contracts, accounts, hybrid OracleProxy / centralized Oracle Worker readiness, Walrus publisher, subgraph, SP1 guest proof fixtures, and drop-script build readiness.
---

# Drop Script Environment Check

Use the project checklist script before running `drop-script` end to end.

Default command:

```sh
drop-script/scripts/check-env.sh
```

Useful scoped commands:

```sh
drop-script/scripts/check-env.sh --section tools
drop-script/scripts/check-env.sh --section env
drop-script/scripts/check-env.sh --section accounts
drop-script/scripts/check-env.sh --section contracts
drop-script/scripts/check-env.sh --section sp1
drop-script/scripts/check-env.sh --section walrus
drop-script/scripts/check-env.sh --section oracle
drop-script/scripts/check-env.sh --section subgraph
drop-script/scripts/check-env.sh --section drop-script
drop-script/scripts/check-env.sh --section manual
```

Automation options:

```sh
drop-script/scripts/check-env.sh --strict
drop-script/scripts/check-env.sh --json
```

Interpretation:

- `PASS`: ready.
- `WARN`: not automatically blocking, but confirm before full-flow testing.
- `ACTION_REQUIRED`: blocking or manual user action required.

Safety rules:

- The default checklist is readonly.
- It must not send transactions, deploy contracts, deploy subgraph, request SP1 proofs, run guest execute, or upload Walrus blobs.
- Never print `.env` secrets, deploy keys, API keys, or private keys.
- If a check needs external network access and sandboxing blocks it, rerun that same command with approval.

Manual gates that remain user-owned:

- Confirm Arbitrum Sepolia chain id `421614`.
- Confirm Walrus publisher is mainnet and `/home/justin/walrus/start.sh` is running. For setup or diagnosis, use the `walrus-publisher-setup` skill first.
- Confirm Walrus/Sui balance or storage quota.
- Confirm seller, buyer, and SP1 keys in `drop-script/.env` are allowed for the test.
- Confirm SP1 Prove Network key has balance and PROVE allowance. For detailed operator guidance, use the `prove-network-credit-check` skill.
- Confirm centralized Oracle Worker is deployed and configured with its signer key.
- Confirm `OracleProxy.centralizedOracleSigner()` matches the Worker signer.
- Confirm Worker signer has enough Arbitrum Sepolia ETH.
- Confirm Worker status page reports ready without exposing balances or secrets.
- Before Worker deployment, `ORACLE_MODE` should remain unset or `external`; the warning is expected and prevents accidental Worker calls.
- Confirm subgraph Studio slug and deploy key are valid.

When the user asks to run the full `drop-script` flow, run this checklist first unless they explicitly waive it. If any `ACTION_REQUIRED` remains, report the exact items and wait for the user to handle or approve the next step.
