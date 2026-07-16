# Fair File Marketplace

Buyer-facing TrustDrop file mall web app.

This app is a pure Vite/TypeScript web app. It does not use Tauri.

## Commands

```sh
pnpm --dir app/gui dev
pnpm --dir app/gui build
```

## Cloudflare Pages

Deploy the built frontend with:

```sh
scripts/deploy-ffm-cloudflare-pages.sh
```

Optional environment variables:

```sh
CLOUDFLARE_PAGES_PROJECT=fair-file-marketplace
CLOUDFLARE_PAGES_BRANCH=dev
VITE_TRUSTDROP_VISION_REGISTRY_ADDRESS=0x79A070bF4b64f815249F4ac0ea05bdB983b92261
VITE_TRUSTDROP_IPFS_GATEWAY=https://ipfs.io/ipfs/
```

## Vision File

Vision files describe externally updated content rules:

- `recommendations.featuredAssets`: homepage recommendation ordering.
- `moderation.startTimestamp`: hide marketplace/search assets listed before this Unix timestamp.
- `moderation.minimumListedBlock`: hide marketplace/search assets listed before this block.
- `moderation.blacklistedAssets`: hide specific assets.
- `moderation.blacklistedSellers`: hide all assets from a seller once seller ownership is indexed.

Buyer records are product behavior, not vision configuration: purchased assets remain visible in records and hidden assets are marked there.
