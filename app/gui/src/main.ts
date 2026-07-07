import {
  connectWallet,
  listLocalThreads,
  preparePurchase,
  saleDisplayTitle,
  salePriceEth,
  submitPurchase,
  TrustDropSubgraph,
  upsertLocalThread,
  type BrowserWallet,
  type BuyerThread,
  type MarketplacePurchase,
  type MarketplaceRefund,
  type MarketplaceSale,
  type MarketplaceSettlement,
} from "../../../packages/drop-ts-sdk/src";
import { filterBuyerActivityForContentEngine, filterBuyerThreadsForContentEngine, filterSalesForContentEngine } from "./content-engine/engine";

type Route = "home" | "browse" | "records" | "detail";

type UiState = {
  route: Route;
  query: string;
  tag: string;
  selectedSaleId: string;
  sales: MarketplaceSale[];
  purchases: MarketplacePurchase[];
  settlements: MarketplaceSettlement[];
  refunds: MarketplaceRefund[];
  localThreads: BuyerThread[];
  wallet: BrowserWallet | null;
  loading: boolean;
  purchaseBusy: boolean;
  message: string;
};

const subgraph = new TrustDropSubgraph();

const state: UiState = {
  route: "home",
  query: "",
  tag: "all",
  selectedSaleId: "",
  sales: [],
  purchases: [],
  settlements: [],
  refunds: [],
  localThreads: [],
  wallet: null,
  loading: true,
  purchaseBusy: false,
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
    state.sales = filterSalesForContentEngine(await subgraph.listSales());
    if (!state.sales.some((sale) => sale.id === state.selectedSaleId)) {
      state.selectedSaleId = state.sales[0]?.id || "";
    }
    state.localThreads = filterBuyerThreadsForContentEngine(await listLocalThreads(), state.sales);
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
  const filtered = filterBuyerActivityForContentEngine(activity, state.sales);
  state.purchases = filtered.purchases;
  state.settlements = filtered.settlements;
  state.refunds = filtered.refunds;
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
        <h1>Records</h1>
        <p>${state.wallet ? `${state.purchases.length} purchase events indexed for ${shortAddress(state.wallet.account)}.` : "Wallet-scoped purchase history."}</p>
      </div>
      <button class="text-button" id="refresh-button" type="button">Refresh</button>
    </section>
    ${state.message ? `<div class="notice">${escapeHtml(state.message)}</div>` : ""}
    <section class="record-list">${state.loading ? loadingRows() : merged}</section>
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
  const settlementKeys = new Set(state.settlements.map((item) => `${item.channel}:${item.saleId}:${item.buyer}`.toLowerCase()));
  const refundKeys = new Set(state.refunds.map((item) => `${item.channel}:${item.saleId}:${item.buyer}`.toLowerCase()));
  const indexedRows = state.purchases.map((purchase) => {
    const sale = state.sales.find((item) => item.channel.toLowerCase() === purchase.channel.toLowerCase() && item.saleId.toLowerCase() === purchase.saleId.toLowerCase());
    const key = `${purchase.channel}:${purchase.saleId}:${purchase.buyer}`.toLowerCase();
    const status = settlementKeys.has(key) ? "Settled" : refundKeys.has(key) ? "Refunded" : "Waiting for seller";
    return `
      <article class="record">
        <div>
          <h2>${escapeHtml(sale ? saleDisplayTitle(sale) : shortAddress(purchase.saleId))}</h2>
          <p>${shortAddress(purchase.txHash)} · ${formatTimestamp(purchase.timestamp)}</p>
        </div>
        <span class="status">${status}</span>
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
        <span class="status">${thread.status.split("_").join(" ")}</span>
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
      updatedAt: Date.now(),
    });
    await refreshBuyerActivity();
    state.localThreads = filterBuyerThreadsForContentEngine(await listLocalThreads(), state.sales);
    state.message = `Purchase submitted ${shortAddress(txHash)}.`;
  } catch (error) {
    state.message = errorMessage(error);
  } finally {
    state.purchaseBusy = false;
    render();
  }
}

function purchaseStatusText(): string {
  if (!state.wallet) return "Wallet required";
  if (state.purchaseBusy) return "Submitting purchase";
  return "Ready";
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
