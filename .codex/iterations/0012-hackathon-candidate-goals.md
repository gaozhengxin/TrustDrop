# 0012 Hackathon Candidate Goals

## Background

TrustDrop is preparing an ETHGlobal Lisbon application under the "hack on existing project" path. The project is already a testnet prototype, not a from-scratch build.

This document records candidate development goals for a possible hackathon phase. These are working suggestions only. They are not final decisions until the project owner confirms the scope.

## Current State

TrustDrop currently has:

- A buyer-facing Fair File Marketplace web app.
- Arbitrum Sepolia contracts.
- SP1 zk verifier contracts for the protocol proof flows.
- Walrus-backed encrypted file delivery.
- A seller CLI and daemon flow.
- Subgraph indexing.
- Public portal, docs, and source snapshot.

The deployment is for testing and demo use. It is not a production product and has no real users yet.

## Candidate Goal

Use the hackathon period to make TrustDrop easier to evaluate as a product and protocol demo, without presenting existing work as newly built.

The suggested direction is to improve clarity, reliability, and demo quality around the existing fair file exchange flow.

## Candidate Scope

Possible work items:

- Improve the buyer purchase, recovery, download, and decrypt flow.
- Add clearer transaction status display for buyer records.
- Improve seller CLI and daemon visibility for purchase detection, fulfill, settle, and failure recovery.
- Add better diagnostics around file availability and recoverability.
- Prepare a reproducible public testnet sale for judges or reviewers.
- Tighten documentation so a reviewer can understand the protocol, app, contracts, zk programs, and seller tooling quickly.
- If sponsor feedback indicates a better fit, add one small integration or UX improvement during the event.

## Non-Goals

- Do not claim the whole protocol was built during the hackathon.
- Do not make the submission purely Walrus-focused unless the prize rules require that framing.
- Do not change core cryptographic protocol design just for demo polish.
- Do not rebuild unrelated frontend, contract, or CLI subsystems without a clear event-scoped reason.

## Application Framing Notes

For ETHGlobal application answers:

- State that TrustDrop is an existing testnet prototype.
- State that it is live for testing and demos, but not production and has no real users.
- Keep the "current state" answer concise.
- Use the "what will you add" answer to describe candidate improvements.
- Be explicit that new hackathon work will be separated in commits and documentation.

## Open Questions

- Does the Lisbon Sui prize accept existing projects with event-period improvements?
- Does the prize require Walrus Devnet, or is Walrus mainnet acceptable?
- Is TrustDrop better positioned as a product demo, protocol demo, or infrastructure integration?
- Which one visible feature should be committed to if the application is accepted?

## Experience Notes

The application should avoid overfitting to a sponsor before the rules are confirmed. TrustDrop's strongest story is fair exchange for paid digital files; Walrus is an important part of that story, but not the whole product.
