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
```
