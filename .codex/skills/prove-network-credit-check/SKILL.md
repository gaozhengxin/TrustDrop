---
name: prove-network-credit-check
description: Use when guiding a TrustDrop seller/operator to prepare SP1 Prove Network credit for proof requests, including what the agent may check locally, what the seller must confirm in dashboard/wallet, and how to avoid wasting proof requests.
---

# Prove Network Credit Guidance

Use this skill when an agent needs to guide a seller/operator through SP1 Prove Network credit readiness before any TrustDrop workflow that may request proofs:

- `guest/scripts/zk-proof-test.sh <vss|vdd> prove`
- `guest/scripts/zk-proof-test.sh <vss|vdd> all`
- `drop-cli proof vss|vdd ... --yes`
- `drop-cli phase prove ... --yes`
- `drop-cli` full-flow scripts with `--yes-prove`

## Safety Rules

- Do not request a proof while checking credit readiness.
- Do not run local proving during development; TrustDrop uses SP1 Prove Network for proof generation.
- Do not print private keys, `.env` contents, dashboard tokens, or API secrets.
- Do not manage, transfer, approve, deposit, or withdraw PROVE on behalf of the seller unless the seller gives an explicit transaction-level instruction.
- Do not ask the seller to reveal exact private balances unless it is necessary; ask them to confirm sufficient/insufficient status.
- Do not blindly retry a failed proof request. Diagnose the first failure and stop for user approval.
- Run VSS and VDD proof requests serially unless the user explicitly approves otherwise.
- Treat successful proof requests as potentially credit-consuming even when failed requests may not charge credit.

## Current TrustDrop Environment

TrustDrop proof scripts read:

```sh
TRUSTDROP_ENV=drop-script/.env
SP1_PRIVATE_KEY=...
```

`guest/scripts/zk-proof-test.sh` maps `SP1_PRIVATE_KEY` to `NETWORK_PRIVATE_KEY` for the SP1 SDK when `NETWORK_PRIVATE_KEY` is not already set.

Do not duplicate seller/operator private keys in extra files. Prefer one configured env file and document which workflow reads it.

## Agent Role

The agent's job is to:

- Identify which requester address TrustDrop will use.
- Explain what the seller needs to check in the Succinct dashboard or wallet.
- Ask the seller for confirmation in plain language.
- Record readiness status in the relevant task notes when useful.
- Stop before any proof request if credit readiness is unknown.

The agent should not present itself as the owner of the seller's PROVE funds. The seller controls funding, approvals, deposits, and withdrawals.

## What The Seller Must Have

The seller/proof requester must have:

- An SP1 requester private key configured as `SP1_PRIVATE_KEY` or `NETWORK_PRIVATE_KEY`.
- Enough ETH for any required onchain interactions by the requester wallet.
- Enough PROVE payment capacity for proof requests.
- Any required PROVE approval/deposit completed in the current Succinct Prover Network flow.

Succinct's current payment model uses PROVE for requester/prover payments through the Succinct vApp. Requesters deposit PROVE into the vApp, and unused funds remain escrowed until withdrawn. Because this flow can change, tell the seller to follow the current official Succinct dashboard/docs for the exact deposit and approval UI.

## Readonly Local Checks

The agent may perform readonly local checks.

Check whether the env file exists and has a key name without printing the value:

```sh
rg -n '^(SP1_PRIVATE_KEY|NETWORK_PRIVATE_KEY)=' drop-script/.env
```

Derive the requester address only when needed and only from the configured local env:

```sh
set -a
source drop-script/.env
set +a
cast wallet address --private-key "${SP1_PRIVATE_KEY:-$NETWORK_PRIVATE_KEY}"
```

If using shell history or shared terminals is a concern, ask the seller to derive or confirm the address in their wallet/dashboard instead of running a local private-key command.

Run the project environment checker without proof requests:

```sh
drop-script/scripts/check-env.sh --section sp1
drop-script/scripts/check-env.sh --section accounts
```

Expected limitation: `check-env.sh` warns that PROVE token balance/allowance is not checked automatically. That is intentional unless the current official contract/UI flow is confirmed.

## Seller Dashboard / Wallet Checks

Ask the seller to open the official Succinct dashboard/wallet flow and confirm:

- The requester address shown by the local env matches the address in the Prove Network dashboard.
- The account has sufficient available PROVE credit/deposit for the intended VSS/VDD run.
- Any required PROVE token approval has been completed.
- The account is on the correct network/version for the current SP1 SDK.
- The dashboard shows no pending or stuck request that might confuse the next run.

Use this wording:

```text
Please confirm in the Succinct dashboard/wallet that requester 0x... has enough available PROVE credit/deposit for the next proof request and that any required approval/deposit has already been completed. You do not need to share the exact balance unless you want it recorded.
```

If the seller says credit is not ready, stop. Give only high-level guidance: use the official dashboard/wallet to fund, approve, or deposit as required by the current Succinct flow.

## Before Running A Proof

Before asking the seller to approve a proof request, report:

- Which program will prove: VSS or VDD.
- Which command will run.
- Which requester address will be used.
- Whether this is expected to consume PROVE credit on success.
- Whether a fixture will be overwritten.

Example confirmation:

```text
Ready to request VSS Groth16 proof with requester 0x...
Command: guest/scripts/zk-proof-test.sh vss prove
This may consume PROVE credit on success and update the VSS fixture.
Please confirm that you want me to run this proof request now.
```

## Failure Handling

If a proof request fails:

- Record the exact error.
- Check whether the request appeared in the Prove Network dashboard/explorer.
- If no request appeared, inspect local env, SP1 SDK version, command arguments, and network configuration.
- If the request appeared and failed, inspect public values hash, program/vkey match, and fixture freshness.
- Explain the diagnosis to the seller.
- Stop before retrying unless the seller explicitly approves another request.

## When To Update This Skill

Update this skill when:

- SP1 SDK major version changes.
- Succinct changes its payment/deposit/approval flow.
- TrustDrop changes where it stores `SP1_PRIVATE_KEY`.
- `guest/scripts/zk-proof-test.sh` changes prove command behavior.
