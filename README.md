# TrustDrop

TrustDrop is a prototype protocol for fair exchange of encrypted files. A buyer locks payment on-chain, a seller proves fulfillment with SP1 zk programs, and the buyer can recover the delivered file through the protocol flow.

## Project Map

- `contracts/`: Arbitrum Sepolia contracts, deploy scripts, and Foundry tests.
- `guest/vss/`: SP1 guest for the key-sharing proof.
- `guest/vdd/`: SP1 guest for the data-delivery proof, including the Walrus RSLH/VE path.
- `drop-lib/`: shared Rust cryptography, encoding, CID, and recovery utilities.
- `drop-script/`: integration script for the full prototype flow.
- `drop-cli/`: seller CLI and daemon for listing, responding to purchases, fulfillment, and settlement.
- `packages/drop-ts-sdk/`: buyer-side TypeScript SDK used by the web app.
- `app/gui/`: Fair File Marketplace buyer web app.
- `app/portal/`, `app/docs/`, `app/site/`: TrustDrop portal, docs source, and Cloudflare Pages deploy bundle.
- `oracle-worker/`: centralized prototype oracle worker.
- `subgraph/`: The Graph indexing project.
- `.codex/`: project knowledge base, iteration notes, runbooks, and agent skills.

## Agent Starting Points

Read `.codex/README.md` first for project context and current workflow rules. Use `.codex/docs/` for architecture and operational notes, `.codex/iterations/` for recent design history, and `.codex/skills/` for repeatable testing or environment checks.

For code exploration, start from the package that matches the task instead of scanning the whole repository. The main active paths are `contracts/`, `guest/vss/`, `guest/vdd/`, `drop-cli/`, `drop-script/`, `packages/drop-ts-sdk/`, and `app/gui/`.
