import {
  buyerAssetStatus,
  checkWalrusAggregator,
  connectWallet,
  fileKind,
  getStoredWalrusAggregatorUrl,
  recoverPurchasedAsset,
  listLocalThreads,
  preparePurchase,
  saleDisplayTitle,
  salePriceEth,
  setStoredWalrusAggregatorUrl,
  submitPurchase,
  TrustDropSubgraph,
  upsertLocalThread,
  WALRUS_AGGREGATOR_PRESETS,
  type BrowserWallet,
  type BuyerThread,
  type BuyerKeyMode,
  type DataKeyShare,
  type MarketplacePurchase,
  type MarketplaceRefund,
  type MarketplaceSale,
  type MarketplaceSettlement,
  type VddProof,
} from "../../../packages/drop-ts-sdk/src";
import { filterSalesForContentEngine } from "./content-engine/engine";

type Route = "home" | "browse" | "records" | "settings" | "detail";

type UiState = {
  route: Route;
  query: string;
  tag: string;
  selectedSaleId: string;
  allSales: MarketplaceSale[];
  sales: MarketplaceSale[];
  purchases: MarketplacePurchase[];
  settlements: MarketplaceSettlement[];
  refunds: MarketplaceRefund[];
  dataKeyShares: DataKeyShare[];
  vddProofs: VddProof[];
  localThreads: BuyerThread[];
  wallet: BrowserWallet | null;
  loading: boolean;
  purchaseBusy: boolean;
  downloadBusy: string;
  keyMode: BuyerKeyMode;
  aggregatorUrl: string;
  aggregatorStatus: string;
  message: string;
};

const subgraph = new TrustDropSubgraph();

const state: UiState = {
  route: "home",
  query: "",
  tag: "all",
  selectedSaleId: "",
  allSales: [],
  sales: [],
  purchases: [],
  settlements: [],
  refunds: [],
  dataKeyShares: [],
  vddProofs: [],
  localThreads: [],
  wallet: null,
  loading: true,
  purchaseBusy: false,
  downloadBusy: "",
  keyMode: "wallet_derived",
  aggregatorUrl: getStoredWalrusAggregatorUrl(),
  aggregatorStatus: "",
  message: "",
};

async function boot(): Promise<void> {
  await refreshMarketplace();
  render();
}

async function refreshMarketplace(): Promise<void> {
  state.loading = true;
  render();
  try {
    state.allSales = await subgraph.listSales();
    state.sales = filterSalesForContentEngine(state.allSales);
    if (!state.sales.some((sale) => sale.id === state.selectedSaleId)) {
      state.selectedSaleId = state.sales[0]?.id || "";
    }
    state.localThreads = await listLocalThreads();
    if (state.wallet) await refreshBuyerActivity();
    state.message = "";
  } catch (error) {
    state.message = errorMessage(error);
  } finally {
    state.loading = false;
  }
}

async function refreshBuyerActivity(): Promise<void> {
  if (!state.wallet) return;
  const activity = await subgraph.listBuyerActivity(state.wallet.account);
  state.purchases = activity.purchases;
  state.settlements = activity.settlements;
  state.refunds = activity.refunds;
  state.dataKeyShares = activity.dataKeyShares;
  state.vddProofs = activity.vddProofs;
}

function byScore(sale: MarketplaceSale): number {
  return Number(sale.purchaseCount) * 4 + Number(sale.settlementCount) * 6 + Number(sale.listedAtTimestamp) / 1_000_000_000;
}

function filteredSales(): MarketplaceSale[] {
  const query = state.query.trim().toLowerCase();
  return state.sales
    .filter((sale) => sale.status === "LISTED")
    .filter((sale) => state.tag === "all" || sale.normalizedTags.includes(state.tag))
    .filter((sale) => {
      if (!query) return true;
      return [saleDisplayTitle(sale), sale.description, sale.tags.join(" "), sale.normalizedTags.join(" ")]
        .join(" ")
        .toLowerCase()
        .includes(query);
    });
}

function tagOptions(): string[] {
  return ["all", ...Array.from(new Set(state.sales.flatMap((sale) => sale.normalizedTags))).filter(Boolean)];
}

function selectedSale(): MarketplaceSale | null {
  return state.sales.find((sale) => sale.id === state.selectedSaleId) ?? state.sales[0] ?? null;
}

function renderShell(content: string): string {
  return `
    <header class="topbar">
      <button class="brand" data-route="home" type="button">
        <span class="brand-mark">FF</span>
        <span>Fair File Marketplace</span>
      </button>
      <nav class="nav">
        ${navButton("home", "Home")}
        ${navButton("browse", "Browse")}
        ${navButton("records", "Records")}
        ${navButton("settings", "Settings")}
      </nav>
      <button class="wallet" id="wallet-button" type="button">${state.wallet ? shortAddress(state.wallet.account) : "Connect wallet"}</button>
    </header>
    <main class="layout">${content}</main>
  `;
}

function navButton(route: Route, label: string): string {
  const active = state.route === route ? " active" : "";
  return `<button class="nav-button${active}" data-route="${route}" type="button">${label}</button>`;
}

function renderHome(): string {
  const recommended = [...state.sales].sort((a, b) => byScore(b) - byScore(a)).slice(0, 2);
  return renderShell(`
    <section class="toolbar">
      <div>
        <h1>Fair file transactions</h1>
        <p>Browse seller listings with on-chain purchase, fulfillment, and recovery records.</p>
      </div>
      ${searchBox()}
    </section>
    ${state.message ? `<div class="notice">${escapeHtml(state.message)}</div>` : ""}
    <section class="section">
      <div class="section-title">
        <h2>Recommended</h2>
        <button class="text-button" data-route="browse" type="button">View all</button>
      </div>
      <div class="asset-grid">${state.loading ? loadingRows() : recommended.map(assetCard).join("") || empty("No active listings.")}</div>
    </section>
    <section class="section">
      <div class="section-title"><h2>Latest listings</h2></div>
      <div class="asset-table">${state.loading ? loadingRows() : assetRows([...state.sales].sort((a, b) => Number(b.listedAtTimestamp) - Number(a.listedAtTimestamp)))}</div>
    </section>
  `);
}

function renderBrowse(): string {
  const current = filteredSales();
  return renderShell(`
    <section class="toolbar compact">
      <div>
        <h1>Browse</h1>
        <p>${current.length} active resources on Arbitrum Sepolia.</p>
      </div>
      ${searchBox()}
    </section>
    ${state.message ? `<div class="notice">${escapeHtml(state.message)}</div>` : ""}
    <section class="browse-panel">
      <aside class="filters">
        <h2>Tags</h2>
        ${tagOptions()
          .map((tag) => `<button class="filter${state.tag === tag ? " active" : ""}" data-tag="${escapeAttr(tag)}" type="button">${escapeHtml(tag)}</button>`)
          .join("")}
      </aside>
      <div class="asset-table wide">${state.loading ? loadingRows() : assetRows(current)}</div>
    </section>
  `);
}

function renderRecords(): string {
  const merged = state.wallet ? buyerRecordRows() : empty("Connect wallet to view purchase records.");
  return renderShell(`
    <section class="toolbar compact">
      <div>
        <h1>Assets</h1>
        <p>${state.wallet ? `${state.purchases.length} purchase events indexed for ${shortAddress(state.wallet.account)}.` : "Wallet-scoped purchase history."}</p>
      </div>
      <button class="text-button" id="refresh-button" type="button">Refresh</button>
    </section>
    ${state.message ? `<div class="notice">${escapeHtml(state.message)}</div>` : ""}
    <section class="record-list">${state.loading ? loadingRows() : merged}</section>
  `);
}

function renderSettings(): string {
  return renderShell(`
    <section class="toolbar compact">
      <div>
        <h1>Settings</h1>
        <p>Walrus Mainnet aggregator for buyer downloads.</p>
      </div>
      <button class="text-button" id="check-aggregator-button" type="button">Check</button>
    </section>
    ${state.message ? `<div class="notice">${escapeHtml(state.message)}</div>` : ""}
    <section class="settings-panel">
      <label class="field">
        <span>Preset aggregator</span>
        <select id="aggregator-preset">
          <option value="">Custom</option>
          ${WALRUS_AGGREGATOR_PRESETS.map(
            (preset) => `<option value="${escapeAttr(preset.url)}" ${preset.url === state.aggregatorUrl ? "selected" : ""}>${escapeHtml(preset.name)}</option>`,
          ).join("")}
        </select>
      </label>
      <label class="field">
        <span>Aggregator URL</span>
        <input id="aggregator-url" type="url" value="${escapeAttr(state.aggregatorUrl)}" />
      </label>
      <button class="primary" id="save-settings-button" type="button">Save</button>
      <p class="settings-status">${escapeHtml(state.aggregatorStatus || "Not checked")}</p>
    </section>
  `);
}

function renderDetail(sale: MarketplaceSale): string {
  return renderShell(`
    <section class="detail">
      <button class="text-button" data-route="browse" type="button">Back to browse</button>
      <div class="detail-grid">
        <div class="file-visual">
          <span>${escapeHtml(initials(saleDisplayTitle(sale)))}</span>
        </div>
        <div class="detail-copy">
          <h1>${escapeHtml(saleDisplayTitle(sale))}</h1>
          <p>${escapeHtml(sale.description || sale.info || "No description")}</p>
          <div class="tag-row">${sale.tags.map((tag) => `<span>${escapeHtml(tag)}</span>`).join("") || `<span>untagged</span>`}</div>
          <dl class="facts">
            <div><dt>Price</dt><dd>${salePriceEth(sale)} ETH</dd></div>
            <div><dt>Size</dt><dd>${formatBytes(sale.fileSize)}</dd></div>
            <div><dt>Seller channel</dt><dd>${shortAddress(sale.channel)}</dd></div>
            <div><dt>Settlements</dt><dd>${sale.settlementCount}</dd></div>
            <div><dt>Sale</dt><dd>${shortAddress(sale.saleId)}</dd></div>
            <div><dt>Version</dt><dd>${shortAddress(sale.version)}</dd></div>
          </dl>
          <div class="purchase-panel">
            <div>
              <strong>${purchaseStatusText()}</strong>
              <p>${state.message ? escapeHtml(state.message) : `Listed ${formatTimestamp(sale.listedAtTimestamp)}.`}</p>
            </div>
            <label class="key-mode">
              <span>Key</span>
              <select id="key-mode-select">
                <option value="wallet_derived" ${state.keyMode === "wallet_derived" ? "selected" : ""}>Wallet derived</option>
                <option value="manual_secret" ${state.keyMode === "manual_secret" ? "selected" : ""}>Manual secret</option>
              </select>
            </label>
            <button class="primary" id="purchase-button" type="button" ${state.purchaseBusy ? "disabled" : ""}>
              ${state.purchaseBusy ? "Submitting" : state.wallet ? "Purchase" : "Connect wallet"}
            </button>
          </div>
        </div>
      </div>
    </section>
  `);
}

function searchBox(): string {
  return `
    <label class="search">
      <span>Search</span>
      <input id="search-input" type="search" value="${escapeAttr(state.query)}" placeholder="title, tag, description" />
    </label>
  `;
}

function assetCard(sale: MarketplaceSale): string {
  const title = saleDisplayTitle(sale);
  return `
    <article class="asset-card" data-asset="${escapeAttr(sale.id)}">
      <div class="file-tile">${escapeHtml(initials(title))}</div>
      <div>
        <h2>${escapeHtml(title)}</h2>
        <p>${escapeHtml(sale.description || sale.info || "No description")}</p>
        <div class="meta">${salePriceEth(sale)} ETH · ${formatBytes(sale.fileSize)} · ${sale.settlementCount} settled</div>
      </div>
    </article>
  `;
}

function assetRows(items: MarketplaceSale[]): string {
  if (items.length === 0) return empty("No matching resources.");
  return items
    .map((sale) => {
      const title = saleDisplayTitle(sale);
      return `
        <button class="asset-row" data-asset="${escapeAttr(sale.id)}" type="button">
          <span class="file-badge">${escapeHtml(initials(title))}</span>
          <span class="asset-main">
            <strong>${escapeHtml(title)}</strong>
            <small>${escapeHtml(sale.tags.join(", ") || "untagged")}</small>
          </span>
          <span>${salePriceEth(sale)} ETH</span>
          <span>${sale.purchaseCount} purchases</span>
          <span>${formatTimestamp(sale.listedAtTimestamp)}</span>
        </button>
      `;
    })
    .join("");
}

function buyerRecordRows(): string {
  if (state.purchases.length === 0 && state.localThreads.length === 0) return empty("No purchase records.");
  const refundKeys = new Set(state.refunds.map((item) => `${item.channel}:${item.saleId}:${item.buyer}`.toLowerCase()));
  const indexedRows = state.purchases.map((purchase) => {
    const sale = findSaleForPurchase(purchase);
    const key = `${purchase.channel}:${purchase.saleId}:${purchase.buyer}`.toLowerCase();
    const status = refundKeys.has(key) ? "refunded" : buyerAssetStatus(purchase, state.settlements, state.dataKeyShares);
    const title = sale ? saleDisplayTitle(sale) : shortAddress(purchase.saleId);
    const canDownload = sale && state.wallet && status === "ready_to_download";
    return `
      <article class="record">
        <span class="file-badge kind-${sale ? fileKind(sale.contentType, sale.fileName) : "binary"}">${escapeHtml(fileKindLabel(sale))}</span>
        <div class="record-main">
          <h2>${escapeHtml(title)}</h2>
          <p>${shortAddress(purchase.txHash)} · ${formatTimestamp(purchase.timestamp)}</p>
        </div>
        <span class="status">${statusText(status)}</span>
        <button class="text-button" data-download="${escapeAttr(purchase.txHash)}" type="button" ${canDownload && state.downloadBusy !== purchase.txHash ? "" : "disabled"}>
          ${state.downloadBusy === purchase.txHash ? "Working" : "Download"}
        </button>
      </article>
    `;
  });
  const localRows = state.localThreads.map(
    (thread) => `
      <article class="record">
        <div>
          <h2>${escapeHtml(thread.title)}</h2>
          <p>${shortAddress(thread.txHash)} · local</p>
        </div>
        <span class="status">${statusText(thread.status)}</span>
      </article>
    `,
  );
  return [...indexedRows, ...localRows].join("");
}

function render(): void {
  const root = document.querySelector<HTMLDivElement>("#app");
  if (!root) return;

  if (state.route === "browse") {
    root.innerHTML = renderBrowse();
  } else if (state.route === "records") {
    root.innerHTML = renderRecords();
  } else if (state.route === "settings") {
    root.innerHTML = renderSettings();
  } else if (state.route === "detail") {
    const sale = selectedSale();
    root.innerHTML = sale ? renderDetail(sale) : renderShell(empty("No listing selected."));
  } else {
    root.innerHTML = renderHome();
  }

  bindEvents(root);
}

function bindEvents(root: HTMLElement): void {
  root.querySelectorAll<HTMLButtonElement>("[data-route]").forEach((button) => {
    button.addEventListener("click", () => {
      state.route = (button.dataset.route as Route) ?? "home";
      state.message = "";
      render();
    });
  });

  root.querySelectorAll<HTMLElement>("[data-asset]").forEach((item) => {
    item.addEventListener("click", () => {
      state.selectedSaleId = item.dataset.asset ?? state.sales[0]?.id ?? "";
      state.route = "detail";
      state.message = "";
      render();
    });
  });

  root.querySelectorAll<HTMLButtonElement>("[data-tag]").forEach((button) => {
    button.addEventListener("click", () => {
      state.tag = button.dataset.tag ?? "all";
      render();
    });
  });

  root.querySelector<HTMLInputElement>("#search-input")?.addEventListener("input", (event) => {
    const input = event.target as HTMLInputElement;
    state.query = input.value;
    if (state.route === "home") state.route = "browse";
    render();
    const nextInput = document.querySelector<HTMLInputElement>("#search-input");
    if (nextInput) {
      nextInput.focus();
      const cursor = state.query.length;
      nextInput.setSelectionRange(cursor, cursor);
    }
  });

  root.querySelector<HTMLButtonElement>("#wallet-button")?.addEventListener("click", () => {
    void connect();
  });
  root.querySelector<HTMLButtonElement>("#refresh-button")?.addEventListener("click", () => {
    void refreshMarketplace();
  });
  root.querySelector<HTMLButtonElement>("#purchase-button")?.addEventListener("click", () => {
    void handlePurchase();
  });
  root.querySelector<HTMLSelectElement>("#key-mode-select")?.addEventListener("change", (event) => {
    state.keyMode = (event.target as HTMLSelectElement).value as BuyerKeyMode;
  });
  root.querySelector<HTMLSelectElement>("#aggregator-preset")?.addEventListener("change", (event) => {
    const value = (event.target as HTMLSelectElement).value;
    if (!value) return;
    state.aggregatorUrl = value;
    const input = root.querySelector<HTMLInputElement>("#aggregator-url");
    if (input) input.value = value;
  });
  root.querySelector<HTMLButtonElement>("#save-settings-button")?.addEventListener("click", () => {
    void saveSettings(root);
  });
  root.querySelector<HTMLButtonElement>("#check-aggregator-button")?.addEventListener("click", () => {
    void checkAggregator(root);
  });
  root.querySelectorAll<HTMLButtonElement>("[data-download]").forEach((button) => {
    button.addEventListener("click", () => {
      void handleDownload(button.dataset.download as `0x${string}`);
    });
  });
}

async function connect(): Promise<void> {
  try {
    state.wallet = await connectWallet();
    await refreshBuyerActivity();
    state.message = "";
  } catch (error) {
    state.message = errorMessage(error);
  }
  render();
}

async function handlePurchase(): Promise<void> {
  const sale = selectedSale();
  if (!sale) return;
  if (!state.wallet) {
    await connect();
    return;
  }
  state.purchaseBusy = true;
  state.message = "";
  render();
  try {
    const prepared = await preparePurchase(sale, state.wallet.account, state.wallet.client);
    const txHash = await submitPurchase(prepared, state.wallet.client);
    await upsertLocalThread({
      id: `${sale.channel.toLowerCase()}:${sale.saleId.toLowerCase()}:${txHash.toLowerCase()}`,
      saleId: sale.saleId,
      channel: sale.channel,
      buyer: state.wallet.account,
      title: saleDisplayTitle(sale),
      txHash,
      status: "purchase_seen",
      keyMode: state.keyMode,
      updatedAt: Date.now(),
    });
    await refreshBuyerActivity();
    state.localThreads = await listLocalThreads();
    state.message = `Purchase submitted ${shortAddress(txHash)}.`;
  } catch (error) {
    state.message = errorMessage(error);
  } finally {
    state.purchaseBusy = false;
    render();
  }
}

async function handleDownload(txHash: `0x${string}`): Promise<void> {
  const purchase = state.purchases.find((item) => item.txHash.toLowerCase() === txHash.toLowerCase());
  if (!purchase || !state.wallet) return;
  const sale = findSaleForPurchase(purchase);
  if (!sale) {
    state.message = "Sale metadata is missing for this purchase.";
    render();
    return;
  }
  const thread = state.localThreads.find((item) => item.txHash.toLowerCase() === txHash.toLowerCase());
  const manualSecret = thread?.keyMode === "manual_secret" ? prompt("Recovery secret hex") : undefined;
  state.downloadBusy = txHash;
  state.message = "";
  render();
  try {
    const result = await recoverPurchasedAsset({
      sale,
      purchase,
      settlements: state.settlements,
      dataKeyShares: state.dataKeyShares,
      vddProofs: state.vddProofs,
      buyer: state.wallet.account,
      walletClient: state.wallet.client,
      aggregatorUrl: state.aggregatorUrl,
      manualSecret: manualSecret ? (manualSecret as `0x${string}`) : undefined,
    });
    triggerBrowserDownload(result.bytes, result.fileName, result.contentType);
    state.message = `Download ready: ${result.fileName}`;
  } catch (error) {
    state.message = errorMessage(error);
    await upsertDownloadError(txHash, state.message);
  } finally {
    state.downloadBusy = "";
    state.localThreads = await listLocalThreads();
    render();
  }
}

async function saveSettings(root: HTMLElement): Promise<void> {
  try {
    const input = root.querySelector<HTMLInputElement>("#aggregator-url");
    state.aggregatorUrl = setStoredWalrusAggregatorUrl(input?.value ?? state.aggregatorUrl);
    state.aggregatorStatus = "Saved";
    state.message = "";
  } catch (error) {
    state.message = errorMessage(error);
  }
  render();
}

async function checkAggregator(root: HTMLElement): Promise<void> {
  try {
    const input = root.querySelector<HTMLInputElement>("#aggregator-url");
    const url = input?.value ?? state.aggregatorUrl;
    await checkWalrusAggregator(url);
    state.aggregatorUrl = setStoredWalrusAggregatorUrl(url);
    state.aggregatorStatus = "Available";
    state.message = "";
  } catch (error) {
    state.aggregatorStatus = "Unavailable";
    state.message = `${errorMessage(error)}. Switch aggregator in Settings.`;
  }
  render();
}

async function upsertDownloadError(txHash: `0x${string}`, message: string): Promise<void> {
  const existing = state.localThreads.find((thread) => thread.txHash.toLowerCase() === txHash.toLowerCase());
  if (!existing) return;
  await upsertLocalThread({
    ...existing,
    lastError: message,
    lastErrorAt: Date.now(),
    updatedAt: Date.now(),
  });
}

function purchaseStatusText(): string {
  if (!state.wallet) return "Wallet required";
  if (state.purchaseBusy) return "Submitting purchase";
  return "Ready";
}

function findSaleForPurchase(purchase: MarketplacePurchase): MarketplaceSale | undefined {
  return state.allSales.find((item) => item.channel.toLowerCase() === purchase.channel.toLowerCase() && item.saleId.toLowerCase() === purchase.saleId.toLowerCase());
}

function statusText(status: string): string {
  return status.split("_").join(" ");
}

function fileKindLabel(sale?: MarketplaceSale): string {
  if (!sale) return "BIN";
  const kind = fileKind(sale.contentType, sale.fileName);
  if (kind === "image") return "IMG";
  if (kind === "video") return "VID";
  if (kind === "audio") return "AUD";
  if (kind === "program") return "APP";
  if (kind === "text") return "TXT";
  if (kind === "data") return "DAT";
  return "BIN";
}

function triggerBrowserDownload(bytes: Uint8Array, fileName: string, contentType: string): void {
  const blob = new Blob([bytes], { type: contentType || "application/octet-stream" });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = fileName || "trustdrop-asset.bin";
  document.body.append(anchor);
  anchor.click();
  anchor.remove();
  URL.revokeObjectURL(url);
}

function empty(message: string): string {
  return `<div class="empty">${escapeHtml(message)}</div>`;
}

function loadingRows(): string {
  return `<div class="empty">Loading</div>`;
}

function shortAddress(value: string): string {
  if (value.length <= 14) return value;
  return `${value.slice(0, 6)}...${value.slice(-4)}`;
}

function initials(title: string): string {
  const clean = title.trim();
  if (!clean) return "FF";
  const parts = clean.split(/\s+/).slice(0, 2);
  return parts.map((part) => part[0]).join("").toUpperCase();
}

function formatTimestamp(value: string): string {
  const timestamp = Number(value);
  if (!Number.isFinite(timestamp) || timestamp <= 0) return "-";
  return new Date(timestamp * 1000).toISOString().slice(0, 10);
}

function formatBytes(value: string): string {
  const size = Number(value);
  if (!Number.isFinite(size) || size <= 0) return "-";
  if (size >= 1024 * 1024) return `${(size / 1024 / 1024).toFixed(1)} MB`;
  if (size >= 1024) return `${(size / 1024).toFixed(1)} KB`;
  return `${size} B`;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function escapeHtml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

function escapeAttr(value: string): string {
  return escapeHtml(value);
}

void boot();
