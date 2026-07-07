# Off-chain Recommendation and Moderation Feed

## Background

Fair File Marketplace will later use the Storail technology stack to add a low-frequency, off-chain recommendation and lightweight content review engine.

This engine is not part of the TrustDrop settlement protocol. It is an optional application-layer policy feed loaded by the marketplace UI by default.

## Goal

Provide a default rule table that helps the app rank content, hide clearly unwanted content, warn users about risky content, and maintain simple blacklist indexes.

The feed should improve marketplace usability without changing the on-chain purchase, fulfill, oracle, settle, or recovery path.

## Scope

The feed may include:

- Recommendation rules, such as weights for purchases, settlements, freshness, tags, channels, and curated lists.
- Review rules, such as blocked tags, blocked sale ids, blocked channel addresses, blocked blob ids, and warning labels.
- Indexes for recommended content, demoted content, blacklisted content, and manually curated content.
- Version metadata, such as rule version, content hash, update time, expiry time, and publisher signature.

The feed should not:

- Decide whether a purchase is valid.
- Block a user from calling the contract.
- Affect zk proof validity.
- Become a dependency of seller fulfill or buyer recovery.
- Override the protocol-level fairness guarantee.

## Client Behavior

The app loads the default feed during normal startup and applies it after fetching marketplace data from the subgraph.

The feed is used for UI behavior only:

- Sort recommended items.
- Hide or demote blacklisted listings by default.
- Show warnings for flagged content.
- Apply simple personalized boosts when local user preferences exist.

Users should eventually be able to disable, replace, or extend the default feed.

## Trust Model

This is a trusted off-chain convenience layer. It should be treated as editorial or policy infrastructure, not protocol infrastructure.

The subgraph remains the source of indexed on-chain marketplace state. The recommendation and review feed only changes how the app presents that state.

## Update Model

The off-chain infrastructure updates the feed at low frequency. Each published feed should be immutable by content hash and include enough metadata for the client to cache and display the active policy version.

The initial implementation can be a signed JSON document fetched by the frontend. Later versions can use Storail distribution and indexing primitives.

## Initial Integration Point

The current frontend recommendation score is a simple local formula based on purchase count, settlement count, and listing time. This feed will replace hardcoded constants with externally supplied rules, while keeping a deterministic local fallback when the feed is unavailable.

