import {
  buyerAssetStatus,
  checkWalrusAggregator,
  connectWallet,
  deriveBuyerSecret,
  encryptedWalrusBlobId,
  fileKind,
  getStoredWalrusAggregatorUrl,
  onWalletAccountsChanged,
  onWalletChainChanged,
  recoverPurchasedAsset,
  reconnectWallet,
  recoverBuyerRecordKeys,
  refundPurchase,
  listLocalThreads,
  preparePurchase,
  readDataKeyCommitment,
  saleDisplayTitle,
  salePriceEth,
  setStoredWalrusAggregatorUrl,
  submitPurchase,
  TrustDropSubgraph,
  upsertLocalThread,
  WALRUS_AGGREGATOR_PRESETS,
  walletFromAccount,
  isWalrusBlobReadable,
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
import { featuredAssetRefs, filterSalesForContentEngine, hiddenReasonsForSale, loadVisionDescriptor, marketplaceQueryBounds } from "./content-engine/engine";
import { loadVideoProof, verifyVideoProof, verifyVideoSamplingSeed, videoProofCalldata, videoProofCid, videoProofCurlCommand, type LoadedVideoProof } from "./video-proof";
import { datasetProofCid, datasetSampleCalldata, datasetSampleCurlCommand, datasetViewerUrl, loadDatasetProof, verifyDatasetSample, type LoadedDatasetProof } from "./dataset-proof";

type Route = "home" | "browse" | "records" | "settings" | "detail" | "certificate" | "dataset-certificate" | "dataset-viewer";
type ImportMetaWithEnv = ImportMeta & {
  env?: {
    DEV?: boolean;
  };
};

type UiState = {
  route: Route;
  query: string;
  tag: string;
  selectedSaleId: string;
  allSales: MarketplaceSale[];
  recommendedSales: MarketplaceSale[];
  sales: MarketplaceSale[];
  purchases: MarketplacePurchase[];
  settlements: MarketplaceSettlement[];
  refunds: MarketplaceRefund[];
  dataKeyShares: DataKeyShare[];
  vddProofs: VddProof[];
  localThreads: BuyerThread[];
  wallet: BrowserWallet | null;
  loading: boolean;
  loadingMoreSales: boolean;
  loadingMoreRecommended: boolean;
  recommendedHasMore: boolean;
  recommendedLoadedCount: number;
  salesHasMore: boolean;
  salesLoadedCount: number;
  purchaseBusy: boolean;
  downloadBusy: string;
  refundBusy: string;
  keyMode: BuyerKeyMode;
  aggregatorUrl: string;
  aggregatorStatus: string;
  visionReady: boolean;
  walletMenuOpen: boolean;
  videoProof: LoadedVideoProof | null;
  videoProofLoading: boolean;
  videoProofError: string;
  videoProofVerification: "idle" | "checking" | "accepted" | "rejected";
  videoProofVerificationDetail: string;
  videoSeedVerification: "idle" | "checking" | "accepted" | "rejected" | "unavailable";
  videoSeedVerificationDetail: string;
  datasetProof: LoadedDatasetProof | null;
  datasetProofLoading: boolean;
  datasetProofError: string;
  datasetVerification: Record<"flow" | "clusters", "idle" | "checking" | "accepted" | "rejected">;
  datasetVerificationDetail: Record<"flow" | "clusters", string>;
  message: string;
  expandedRecords: Set<string>;
  recordSecrets: Record<string, { userKey?: string; dataKey?: string }>;
  visibleRecordSecrets: Record<string, { userKey: boolean; dataKey: boolean }>;
  walrusChecks: Record<string, "checking" | "readable" | "unavailable">;
  dataKeyCommitments: Record<string, string>;
};

const subgraph = new TrustDropSubgraph();
const SUBGRAPH_SALES_PAGE_SIZE = 24;
const RECOMMENDED_PAGE_SIZE = 2;
const LATEST_HOME_SIZE = 8;
const ALLOW_MANUAL_SECRET = Boolean((import.meta as ImportMetaWithEnv).env?.DEV);

const state: UiState = {
  route: "home",
  query: "",
  tag: "all",
  selectedSaleId: "",
  allSales: [],
  recommendedSales: [],
  sales: [],
  purchases: [],
  settlements: [],
  refunds: [],
  dataKeyShares: [],
  vddProofs: [],
  localThreads: [],
  wallet: null,
  loading: true,
  loadingMoreSales: false,
  loadingMoreRecommended: false,
  recommendedHasMore: true,
  recommendedLoadedCount: 0,
  salesHasMore: true,
  salesLoadedCount: 0,
  purchaseBusy: false,
  downloadBusy: "",
  refundBusy: "",
  keyMode: "wallet_derived",
  aggregatorUrl: getStoredWalrusAggregatorUrl(),
  aggregatorStatus: "",
  visionReady: false,
  walletMenuOpen: false,
  videoProof: null,
  videoProofLoading: false,
  videoProofError: "",
  videoProofVerification: "idle",
  videoProofVerificationDetail: "",
  videoSeedVerification: "idle",
  videoSeedVerificationDetail: "",
  datasetProof: null,
  datasetProofLoading: false,
  datasetProofError: "",
  datasetVerification: { flow: "idle", clusters: "idle" },
  datasetVerificationDetail: { flow: "", clusters: "" },
  message: "",
  expandedRecords: new Set(),
  recordSecrets: {},
  visibleRecordSecrets: {},
  walrusChecks: {},
  dataKeyCommitments: {},
};

async function boot(): Promise<void> {
  if (window.location.pathname.replace(/\/+$/, "") === "/dataset-viewer") state.route = "dataset-viewer";
  else if (datasetProofRequestFromUrl()) state.route = "dataset-certificate";
  else if (proofRequestFromUrl()) state.route = "certificate";
  installWalletListeners();
  render();
  try {
    state.wallet = await reconnectWallet();
  } catch (error) {
    state.wallet = null;
    state.message = errorMessage(error);
  }
  try {
    await loadVisionDescriptor();
    state.visionReady = true;
  } catch (error) {
    state.visionReady = false;
    state.loading = false;
    state.message = `Content rules unavailable: ${errorMessage(error)}`;
    render();
    return;
  }
  await refreshMarketplace();
  render();
}

async function refreshMarketplace(): Promise<void> {
  state.loading = true;
  state.loadingMoreSales = false;
  state.salesHasMore = true;
  state.salesLoadedCount = 0;
  state.recommendedHasMore = true;
  state.recommendedLoadedCount = 0;
  state.allSales = [];
  state.recommendedSales = [];
  state.sales = [];
  render();
  try {
    await loadMoreMarketplaceSales(false);
    await loadMoreRecommendedSales(false);
    if (state.route === "certificate") await loadRequestedVideoProof();
    if (state.route === "dataset-certificate") await loadRequestedDatasetProof();
    if (!state.sales.some((sale) => sale.id === state.selectedSaleId)) {
      state.selectedSaleId = state.sales[0]?.id || state.recommendedSales[0]?.id || "";
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

async function refreshMarketplaceAndRender(): Promise<void> {
  await refreshMarketplace();
  render();
}

async function loadMoreMarketplaceSales(renderOnFinish = true): Promise<void> {
  if (state.loadingMoreSales || !state.salesHasMore) return;
  state.loadingMoreSales = true;
  if (renderOnFinish) render();
  try {
    const next = await subgraph.listSales({
      first: SUBGRAPH_SALES_PAGE_SIZE,
      skip: state.salesLoadedCount,
      ...marketplaceQueryBounds(),
    });
    upsertAllSales(next);
    state.salesLoadedCount += next.length;
    state.salesHasMore = next.length === SUBGRAPH_SALES_PAGE_SIZE;
    state.sales = filterSalesForContentEngine([...state.sales, ...next])
      .filter(uniqueSale)
      .sort((a, b) => Number(b.listedAtTimestamp) - Number(a.listedAtTimestamp));
    if (!state.sales.some((sale) => sale.id === state.selectedSaleId)) {
      state.selectedSaleId = state.sales[0]?.id || state.recommendedSales[0]?.id || "";
    }
    state.message = "";
  } catch (error) {
    state.message = errorMessage(error);
  } finally {
    state.loadingMoreSales = false;
    if (renderOnFinish) render();
  }
}

async function loadMoreRecommendedSales(renderOnFinish = true): Promise<void> {
  if (state.loadingMoreRecommended || !state.recommendedHasMore) return;
  const refs = featuredAssetRefs();
  if (state.recommendedLoadedCount >= refs.length) {
    state.recommendedHasMore = false;
    return;
  }
  state.loadingMoreRecommended = true;
  if (renderOnFinish) render();
  try {
    const visible: MarketplaceSale[] = [];
    let cursor = state.recommendedLoadedCount;
    while (cursor < refs.length && visible.length < RECOMMENDED_PAGE_SIZE) {
      const ref = refs[cursor];
      await loadMarketplaceUntilRecommendedRefsAreAvailable([ref]);
      const sale = state.sales.find((item) => sameSaleRef(item, ref.channel, ref.saleId));
      if (sale && sale.status === "LISTED" && hiddenReasonsForSale(sale).length === 0) {
        visible.push(sale);
      }
      cursor += 1;
    }
    upsertAllSales(visible);
    state.recommendedSales = [...state.recommendedSales, ...visible].filter(uniqueSale);
    state.recommendedLoadedCount = cursor;
    state.recommendedHasMore = state.recommendedLoadedCount < refs.length;
    if (!state.selectedSaleId) state.selectedSaleId = state.sales[0]?.id || state.recommendedSales[0]?.id || "";
    state.message = "";
  } catch (error) {
    state.message = errorMessage(error);
  } finally {
    state.loadingMoreRecommended = false;
    if (renderOnFinish) render();
  }
}

async function loadMarketplaceUntilRecommendedRefsAreAvailable(refs: Array<{ channel: `0x${string}`; saleId: `0x${string}` }>): Promise<void> {
  while (state.salesHasMore && refs.some((ref) => !state.sales.some((sale) => sameSaleRef(sale, ref.channel, ref.saleId)))) {
    await loadMoreMarketplaceSales(false);
  }
}

function upsertAllSales(sales: MarketplaceSale[]): void {
  const byId = new Map(state.allSales.map((sale) => [sale.id.toLowerCase(), sale]));
  for (const sale of sales) byId.set(sale.id.toLowerCase(), sale);
  state.allSales = Array.from(byId.values());
}

function uniqueSale(sale: MarketplaceSale, index: number, sales: MarketplaceSale[]): boolean {
  return sales.findIndex((item) => item.id.toLowerCase() === sale.id.toLowerCase()) === index;
}

function sameSaleRef(sale: MarketplaceSale, channel: string, saleId: string): boolean {
  return sale.channel.toLowerCase() === channel.toLowerCase() && sale.saleId.toLowerCase() === saleId.toLowerCase();
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

function filteredSales(): MarketplaceSale[] {
  if (!state.visionReady) return [];
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
  if (!state.visionReady) return ["all"];
  return ["all", ...Array.from(new Set(state.sales.flatMap((sale) => sale.normalizedTags))).filter(Boolean)];
}

function selectedSale(): MarketplaceSale | null {
  if (!state.visionReady) return null;
  return state.allSales.find((sale) => sale.id === state.selectedSaleId) ?? state.sales[0] ?? null;
}

function proofRequestFromUrl(): { cid: string; channel: `0x${string}`; saleId: `0x${string}` } | null {
  const params = new URLSearchParams(window.location.search);
  const cid = params.get("proof") ?? "";
  const channel = params.get("channel") ?? "";
  const saleId = params.get("saleId") ?? "";
  if (!cid || !/^0x[0-9a-fA-F]{40}$/.test(channel) || !/^0x[0-9a-fA-F]{64}$/.test(saleId)) return null;
  return { cid, channel: channel as `0x${string}`, saleId: saleId as `0x${string}` };
}

async function loadRequestedVideoProof(): Promise<void> {
  const request = proofRequestFromUrl();
  if (!request) {
    state.videoProofError = "The certificate URL is incomplete.";
    return;
  }
  state.videoProof = null;
  state.videoProofLoading = true;
  state.videoProofError = "";
  state.videoProofVerification = "idle";
  state.videoProofVerificationDetail = "";
  state.videoSeedVerification = "idle";
  state.videoSeedVerificationDetail = "";
  render();
  try {
    let sale = state.allSales.find((item) => sameSaleRef(item, request.channel, request.saleId));
    if (!sale) {
      sale = await subgraph.getSale(request.channel, request.saleId) ?? undefined;
      if (sale) upsertAllSales([sale]);
    }
    if (!sale) throw new Error("The sale referenced by this certificate is not indexed.");
    if (videoProofCid(sale.tags) !== request.cid) throw new Error("The sale does not reference this certificate CID.");
    state.selectedSaleId = sale.id;
    state.videoProof = await loadVideoProof(request.cid, sale);
    const loadedProof = state.videoProof;
    state.videoProofVerification = "checking";
    state.videoSeedVerification = "checking";
    render();
    const proofCheck = (async () => { try {
      await verifyVideoProof(loadedProof.certificate);
      state.videoProofVerification = "accepted";
      state.videoProofVerificationDetail = "The SP1 Groth16 verifier accepted this proof on Arbitrum Sepolia.";
    } catch (error) {
      state.videoProofVerification = "rejected";
      state.videoProofVerificationDetail = errorMessage(error);
    } })();
    const seedCheck = (async () => { try {
      await verifyVideoSamplingSeed(loadedProof.certificate);
      state.videoSeedVerification = "accepted";
      state.videoSeedVerificationDetail = "The certificate uses the latest seed committed by the sampling challenge contract.";
    } catch (error) {
      const detail = errorMessage(error);
      state.videoSeedVerification = detail.includes("predates on-chain") ? "unavailable" : "rejected";
      state.videoSeedVerificationDetail = detail;
    } })();
    await Promise.all([proofCheck, seedCheck]);
  } catch (error) {
    state.videoProofError = errorMessage(error);
  } finally {
    state.videoProofLoading = false;
    render();
  }
}

function datasetProofRequestFromUrl(): { cid: string; channel: `0x${string}`; saleId: `0x${string}` } | null {
  const params = new URLSearchParams(window.location.search);
  const cid = params.get("datasetProof") ?? "";
  const channel = params.get("channel") ?? "";
  const saleId = params.get("saleId") ?? "";
  if (!cid || !/^0x[0-9a-fA-F]{40}$/.test(channel) || !/^0x[0-9a-fA-F]{64}$/.test(saleId)) return null;
  return { cid, channel: channel as `0x${string}`, saleId: saleId as `0x${string}` };
}

async function loadRequestedDatasetProof(): Promise<void> {
  const request = datasetProofRequestFromUrl();
  if (!request) { state.datasetProofError = "The certificate URL is incomplete."; return; }
  state.datasetProof = null;
  state.datasetProofLoading = true;
  state.datasetProofError = "";
  state.datasetVerification = { flow: "idle", clusters: "idle" };
  state.datasetVerificationDetail = { flow: "", clusters: "" };
  render();
  try {
    let sale = state.allSales.find((item) => sameSaleRef(item, request.channel, request.saleId));
    if (!sale) {
      sale = await subgraph.getSale(request.channel, request.saleId) ?? undefined;
      if (sale) upsertAllSales([sale]);
    }
    if (!sale) throw new Error("The sale referenced by this certificate is not indexed.");
    if (datasetProofCid(sale.tags) !== request.cid) throw new Error("The sale does not reference this chain-intelligence certificate CID.");
    state.selectedSaleId = sale.id;
    state.datasetProof = await loadDatasetProof(request.cid, sale);
    const loaded = state.datasetProof;
    state.datasetVerification = { flow: "checking", clusters: "checking" };
    render();
    await Promise.all((["flow", "clusters"] as const).map(async (kind) => {
      try {
        await verifyDatasetSample(kind, loaded.certificate);
        state.datasetVerification[kind] = "accepted";
        state.datasetVerificationDetail[kind] = "The standalone SP1 Groth16 verifier accepted this sample proof on Arbitrum Sepolia.";
      } catch (error) {
        state.datasetVerification[kind] = "rejected";
        state.datasetVerificationDetail[kind] = errorMessage(error);
      }
    }));
  } catch (error) {
    state.datasetProofError = errorMessage(error);
  } finally {
    state.datasetProofLoading = false;
    render();
  }
}

function proofPageUrl(sale: MarketplaceSale, cid: string): string {
  const url = new URL(window.location.href);
  url.search = new URLSearchParams({ proof: cid, channel: sale.channel, saleId: sale.saleId }).toString();
  url.hash = "";
  return url.toString();
}

function datasetProofPageUrl(sale: MarketplaceSale, cid: string): string {
  const url = new URL(window.location.href);
  url.search = new URLSearchParams({ datasetProof: cid, channel: sale.channel, saleId: sale.saleId }).toString();
  url.hash = "";
  return url.toString();
}

function clearProofUrl(): void {
  const url = new URL(window.location.href);
  url.searchParams.delete("proof");
  url.searchParams.delete("datasetProof");
  url.searchParams.delete("channel");
  url.searchParams.delete("saleId");
  window.history.replaceState(null, "", url);
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
      <div class="wallet-panel">
        <button class="wallet" id="wallet-button" type="button" aria-expanded="${state.wallet && state.walletMenuOpen ? "true" : "false"}">
          ${state.wallet ? shortAddress(state.wallet.account) : "Connect wallet"}
        </button>
        ${state.wallet && state.walletMenuOpen ? walletMenu() : ""}
      </div>
    </header>
    <main class="layout">
      ${trustdropProtocolNote()}
      ${content}
    </main>
  `;
}

function navButton(route: Route, label: string): string {
  const active = state.route === route ? " active" : "";
  return `<button class="nav-button${active}" data-route="${route}" type="button">${label}</button>`;
}

function trustdropProtocolNote(): string {
  return `
    <section class="protocol-note">
      Fair File Marketplace uses TrustDrop for escrow, proof-backed fulfillment, and buyer recovery.
      <a href="https://trustdrop.pages.dev">Learn about TrustDrop</a>
    </section>
  `;
}

function walletMenu(): string {
  if (!state.wallet) return "";
  const explorerUrl = `https://sepolia.arbiscan.io/address/${state.wallet.account}`;
  return `
    <div class="wallet-menu">
      <div class="wallet-menu-account">
        <span>Arbitrum Sepolia</span>
        <strong>${escapeHtml(shortAddress(state.wallet.account))}</strong>
      </div>
      <button class="wallet-menu-item" id="switch-wallet-button" type="button">Switch account</button>
      <button class="wallet-menu-item" id="copy-wallet-button" type="button">Copy address</button>
      <a class="wallet-menu-item" href="${escapeAttr(explorerUrl)}" target="_blank" rel="noreferrer">View on explorer</a>
      <button class="wallet-menu-item danger" id="disconnect-wallet-button" type="button">Disconnect</button>
    </div>
  `;
}

function renderHome(): string {
  if (!state.visionReady && state.loading) return renderShell(contentRulesLoading());
  if (!state.visionReady) return renderShell(contentRulesUnavailable());
  const latest = state.sales.slice(0, LATEST_HOME_SIZE);
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
        ${state.recommendedHasMore ? `<button class="text-button" id="load-more-recommended-button" type="button" ${state.loadingMoreRecommended ? "disabled" : ""}>${state.loadingMoreRecommended ? "Loading" : "More"}</button>` : ""}
      </div>
      <div class="asset-grid">${state.loading ? loadingRows() : state.recommendedSales.map(assetCard).join("") || empty("No recommended listings.")}</div>
    </section>
    <section class="section">
      <div class="section-title">
        <h2>Latest listings</h2>
        <button class="text-button" data-route="browse" type="button">Browse all</button>
      </div>
      <div class="asset-table">${state.loading ? loadingRows() : assetRows(latest)}</div>
    </section>
  `);
}

function renderBrowse(): string {
  if (!state.visionReady && state.loading) return renderShell(contentRulesLoading());
  if (!state.visionReady) return renderShell(contentRulesUnavailable());
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
      <div>
        <div class="asset-table wide">${state.loading ? loadingRows() : assetRows(current)}</div>
        ${loadMoreSalesControl()}
      </div>
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
  if (!state.visionReady && state.loading) return renderShell(contentRulesLoading());
  if (!state.visionReady) return renderShell(contentRulesUnavailable());
  const certificateCid = videoProofCid(sale.tags);
  const datasetCertificateCid = datasetProofCid(sale.tags);
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
          ${certificateCid ? `<a class="proof-link" href="${escapeAttr(proofPageUrl(sale, certificateCid))}" target="_blank" rel="noreferrer">View video sampling certificate</a>` : ""}
          ${datasetCertificateCid ? `<a class="proof-link" href="${escapeAttr(datasetProofPageUrl(sale, datasetCertificateCid))}" target="_blank" rel="noreferrer">View chain-intelligence sampling certificate</a>` : ""}
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
                ${ALLOW_MANUAL_SECRET ? `<option value="manual_secret" ${state.keyMode === "manual_secret" ? "selected" : ""}>Manual secret (dev)</option>` : ""}
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

function renderCertificate(): string {
  const sale = selectedSale();
  const proof = state.videoProof;
  if (state.videoProofLoading && !proof) {
    return renderShell(`<section class="certificate-page">${loadingRows()}</section>`);
  }
  if (state.videoProofError || !proof || !sale) {
    return renderShell(`
      <section class="certificate-page">
        <button class="text-button" data-route="detail" type="button">Back to listing</button>
        <div class="notice">${escapeHtml(state.videoProofError || "Certificate unavailable")}</div>
      </section>
    `);
  }
  const certificate = proof.certificate;
  const verifierUrl = `https://sepolia.arbiscan.io/address/${certificate.verifier.address}`;
  const verificationLabel = state.videoProofVerification === "accepted" ? "Verified" : state.videoProofVerification === "rejected" ? "Verification failed" : "Verifying";
  const seedLabel = state.videoSeedVerification === "accepted" ? "Seed matched" : state.videoSeedVerification === "rejected" ? "Seed mismatch" : state.videoSeedVerification === "unavailable" ? "Not recorded" : "Checking seed";
  const overallVerification = state.videoProofVerification === "rejected" || state.videoSeedVerification === "rejected"
    ? "rejected"
    : state.videoProofVerification === "accepted" && state.videoSeedVerification === "accepted"
      ? "accepted"
      : state.videoProofVerification === "accepted" && state.videoSeedVerification === "unavailable"
        ? "unavailable"
        : "checking";
  const overallLabel = overallVerification === "accepted" ? "Verified" : overallVerification === "rejected" ? "Certificate warning" : overallVerification === "unavailable" ? "Seed not recorded" : "Verifying";
  return renderShell(`
    <section class="certificate-page">
      <div class="certificate-heading">
        <div>
          <span class="eyebrow">Video sampling certificate</span>
          <h1>${escapeHtml(saleDisplayTitle(sale))}</h1>
          <p>Three sale-derived previews are bound to the plaintext Walrus blob by an SP1 proof.</p>
        </div>
        <div class="certificate-actions">
          <span class="verification-badge ${overallVerification}">${escapeHtml(overallLabel)}</span>
          <button class="text-button" id="download-certificate-button" type="button">Download certificate</button>
        </div>
      </div>
      <div class="preview-grid">
        ${proof.previewUrls.map((url, index) => `
          <article class="preview-card">
            <video controls playsinline preload="metadata" src="${escapeAttr(url)}"></video>
            <div><strong>Preview ${index + 1}</strong><span>Bucket ${certificate.previews[index].bucket}</span></div>
            <code>${escapeHtml(certificate.previews[index].cid)}</code>
          </article>
        `).join("")}
      </div>
      <section class="certificate-panel">
        <div class="section-title"><h2>Certificate checks</h2></div>
        <div class="certificate-check">
          <div><strong>ZK proof</strong><p>${escapeHtml(state.videoProofVerificationDetail || "Calling the verifier contract…")}</p></div>
          <span class="verification-badge ${state.videoProofVerification}">${escapeHtml(verificationLabel)}</span>
        </div>
        <div class="certificate-check">
          <div><strong>Sampling seed</strong><p>${escapeHtml(state.videoSeedVerificationDetail || "Reading the latest on-chain sampling challenge…")}</p></div>
          <span class="verification-badge ${state.videoSeedVerification}">${escapeHtml(seedLabel)}</span>
        </div>
        <dl class="certificate-facts">
          <div><dt>Verifier</dt><dd><a href="${escapeAttr(verifierUrl)}" target="_blank" rel="noreferrer">${escapeHtml(certificate.verifier.address)}</a></dd></div>
          <div><dt>Verifier version</dt><dd>${escapeHtml(certificate.verifier.version)}</dd></div>
          <div><dt>Proof system</dt><dd>${escapeHtml(certificate.proof.system)}</dd></div>
          <div><dt>Program vkey</dt><dd><code>${escapeHtml(certificate.proof.programVKey)}</code></dd></div>
        </dl>
        <details class="verification-reproduce">
          <summary>Reproduce this verification</summary>
          <div class="verification-code-heading"><strong>Calldata</strong></div>
          <pre><code>${escapeHtml(videoProofCalldata(certificate))}</code></pre>
          <div class="verification-code-heading">
            <strong>Terminal command</strong>
            <button class="text-button" id="copy-verification-command-button" type="button">Copy</button>
          </div>
          <p>Paste this command into Terminal. A valid proof prints <code>Verified</code>.</p>
          <pre><code id="verification-command">${escapeHtml(videoProofCurlCommand(certificate))}</code></pre>
        </details>
      </section>
      <section class="certificate-panel">
        <h2>Claimed content</h2>
        <dl class="certificate-facts">
          <div><dt>Plaintext Walrus blob ID</dt><dd><code>${escapeHtml(certificate.origin.walrusBlobId)}</code></dd></div>
          <div><dt>Sale contract</dt><dd><code>${escapeHtml(certificate.sale.contract)}</code></dd></div>
          <div><dt>Sale ID</dt><dd><code>${escapeHtml(certificate.sale.saleId)}</code></dd></div>
          <div><dt>Randomness source</dt><dd>${escapeHtml(certificate.sampling.randomSource)}</dd></div>
          <div><dt>Sampling seed</dt><dd><code>${escapeHtml(certificate.sampling.seed)}</code></dd></div>
          ${certificate.sampling.challenge ? `
            <div><dt>Challenge contract</dt><dd><code>${escapeHtml(certificate.sampling.challenge.contract)}</code></dd></div>
            <div><dt>Challenge key</dt><dd><code>${escapeHtml(certificate.sampling.challenge.challengeKey)}</code></dd></div>
            <div><dt>Request ID</dt><dd><code>${escapeHtml(certificate.sampling.challenge.requestId)}</code></dd></div>
            <div><dt>Requester</dt><dd><code>${escapeHtml(certificate.sampling.challenge.requester)}</code></dd></div>
            <div><dt>Request count</dt><dd>${certificate.sampling.challenge.requestCount}</dd></div>
            <div><dt>Request transaction</dt><dd><code>${escapeHtml(certificate.sampling.challenge.transactionHash)}</code></dd></div>
          ` : ""}
        </dl>
      </section>
    </section>
  `);
}

function renderDatasetCertificate(): string {
  const sale = selectedSale();
  const loaded = state.datasetProof;
  if (state.datasetProofLoading && !loaded) return renderShell(`<section class="certificate-page">${loadingRows()}</section>`);
  if (state.datasetProofError || !loaded || !sale) return renderShell(`<section class="certificate-page"><button class="text-button" data-route="detail" type="button">Back to listing</button><div class="notice">${escapeHtml(state.datasetProofError || "Certificate unavailable")}</div></section>`);
  const certificate = loaded.certificate;
  const labels = { flow: "Flow graph time sample", clusters: "Wallet-cluster evidence sample" } as const;
  const descriptions = {
    flow: `${formatTimestamp(String(certificate.samples.flow.bucketStart))}–${formatTimestamp(String(certificate.samples.flow.bucketEnd))}`,
    clusters: `${certificate.samples.clusters.clusterIds.length} seed-selected clusters with their supporting transfer subgraphs`,
  } as const;
  const overall = Object.values(state.datasetVerification).some((value) => value === "rejected") ? "rejected" : Object.values(state.datasetVerification).every((value) => value === "accepted") ? "accepted" : "checking";
  return renderShell(`
    <section class="certificate-page">
      <div class="certificate-heading">
        <div><span class="eyebrow">Chain-intelligence v2 sampling certificate</span><h1>${escapeHtml(saleDisplayTitle(sale))}</h1><p>Two authenticated samples are reconstructed from Walrus-bound dataset sections: one time-aligned flow bucket and three complete cluster evidence subgraphs.</p></div>
        <div class="certificate-actions"><span class="verification-badge ${overall}">${overall === "accepted" ? "Verified" : overall === "rejected" ? "Verification failed" : "Verifying"}</span><button class="text-button" id="download-dataset-certificate-button" type="button">Download certificate</button></div>
      </div>
      <div class="dataset-sample-grid">
        ${(["flow", "clusters"] as const).map((kind) => {
          const sample = certificate.samples[kind];
          const status = state.datasetVerification[kind];
          return `<article class="certificate-panel dataset-sample-card">
            <div class="section-title"><div><span class="eyebrow">${kind === "flow" ? "entity_flows" : "cluster_subgraphs"}</span><h2>${labels[kind]}</h2></div><span class="verification-badge ${status}">${status === "accepted" ? "Verified" : status === "rejected" ? "Failed" : "Verifying"}</span></div>
            <p>${escapeHtml(descriptions[kind])}</p>
            <a class="proof-link" href="${escapeAttr(datasetViewerUrl(kind, sample.cid))}" target="_blank" rel="noreferrer">Open authenticated sample in explorer</a>
            <dl class="certificate-facts"><div><dt>Sample CID</dt><dd><code>${escapeHtml(sample.cid)}</code></dd></div><div><dt>Origin commitment</dt><dd><code>${escapeHtml(sample.originBlobId)}</code></dd></div><div><dt>Sampling seed</dt><dd><code>${escapeHtml(sample.samplingSeed)}</code></dd></div><div><dt>Verifier</dt><dd><a href="https://sepolia.arbiscan.io/address/${escapeAttr(sample.verifier.address)}" target="_blank" rel="noreferrer">${escapeHtml(sample.verifier.address)}</a></dd></div></dl>
            <p>${escapeHtml(state.datasetVerificationDetail[kind] || "Calling the verifier contract…")}</p>
            <details class="verification-reproduce"><summary>Reproduce this verification</summary><div class="verification-code-heading"><strong>Calldata</strong></div><pre><code>${escapeHtml(datasetSampleCalldata(kind, certificate))}</code></pre><div class="verification-code-heading"><strong>Terminal command</strong><button class="text-button" data-copy-dataset-verification="${kind}" type="button">Copy</button></div><p>Paste this command into Terminal. A valid proof prints <code>Verified</code>.</p><pre><code>${escapeHtml(datasetSampleCurlCommand(kind, certificate))}</code></pre></details>
          </article>`;
        }).join("")}
      </div>
      <section class="certificate-panel"><h2>Dataset claim</h2><dl class="certificate-facts"><div><dt>Schema</dt><dd><code>${escapeHtml(certificate.dataset.schema)}</code></dd></div><div><dt>Chain</dt><dd>${escapeHtml(certificate.dataset.chain)} · Chain ID ${certificate.dataset.chainId}</dd></div><div><dt>Coverage</dt><dd>${formatTimestamp(String(certificate.dataset.firstTimestamp))}–${formatTimestamp(String(certificate.dataset.lastTimestamp))}</dd></div><div><dt>Source transfers</dt><dd>${certificate.dataset.transferCount.toLocaleString()}</dd></div><div><dt>Clustering rule</dt><dd><code>${escapeHtml(certificate.dataset.clusteringMethod)}</code></dd></div><div><dt>Sale contract</dt><dd><code>${escapeHtml(certificate.sale.contract)}</code></dd></div><div><dt>Sale ID</dt><dd><code>${escapeHtml(certificate.sale.saleId)}</code></dd></div></dl></section>
    </section>`);
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

function loadMoreSalesControl(): string {
  if (state.loading) return "";
  if (!state.salesHasMore) return `<p class="load-more-note">All currently indexed listings have been loaded.</p>`;
  return `
    <div class="load-more">
      <button class="text-button" id="load-more-sales-button" type="button" ${state.loadingMoreSales ? "disabled" : ""}>
        ${state.loadingMoreSales ? "Loading..." : "More listings"}
      </button>
      <span>Loads another ${SUBGRAPH_SALES_PAGE_SIZE} listings after the vision cutoff.</span>
    </div>
  `;
}

function buyerRecordRows(): string {
  if (state.purchases.length === 0 && state.localThreads.length === 0) return empty("No purchase records.");
  const refundKeys = new Set(state.refunds.map((item) => `${item.channel}:${item.saleId}:${item.buyer}`.toLowerCase()));
  const indexedTxHashes = new Set(state.purchases.map((purchase) => purchase.txHash.toLowerCase()));
  const indexedRows = state.purchases.map((purchase) => {
    const sale = findSaleForPurchase(purchase);
    const key = `${purchase.channel}:${purchase.saleId}:${purchase.buyer}`.toLowerCase();
    const status = refundKeys.has(key) ? "refunded" : buyerAssetStatus(purchase, state.settlements, state.dataKeyShares);
    const title = sale ? saleDisplayTitle(sale) : shortAddress(purchase.saleId);
    const canDownload = Boolean(state.wallet && status === "ready_to_download");
    const hidden = sale ? hiddenReasonsForSale(sale).length > 0 : false;
    const refundState = refundAvailability(purchase, status);
    const refundBusy = state.refundBusy.toLowerCase() === purchase.txHash.toLowerCase();
    const thread = state.localThreads.find((item) => item.txHash.toLowerCase() === purchase.txHash.toLowerCase());
    const share = dataKeyShareForPurchase(purchase);
    const proof = vddProofForPurchase(purchase);
    const blobId = sale && proof ? encryptedWalrusBlobId(sale, [proof]) : "";
    const detailId = purchase.txHash.toLowerCase();
    const secret = state.recordSecrets[detailId];
    const visible = state.visibleRecordSecrets[detailId] ?? { userKey: false, dataKey: false };
    const keyMode = thread?.keyMode === "manual_secret" ? "User-specified" : thread?.keyMode === "wallet_derived" ? "Wallet-derived" : "Unknown on this browser";
    const walrusState = state.walrusChecks[detailId];
    const walrusLabel = walrusState === "readable" ? "Readable" : walrusState === "unavailable" ? "Unavailable" : walrusState === "checking" ? "Checking…" : "Not checked";
    return `
      <article class="record">
        <span class="file-badge kind-${sale ? fileKind(sale.contentType, sale.fileName) : "binary"}">${escapeHtml(fileKindLabel(sale))}</span>
        <div class="record-main">
          <h2>${escapeHtml(title)}${hidden ? ` <span class="moderation-badge">Hidden</span>` : ""}</h2>
          <p>${shortAddress(purchase.txHash)} · ${formatTimestamp(purchase.timestamp)}</p>
          <p class="record-guarantees">Fulfill by ${formatTimestamp(purchase.deadline)} · Download guaranteed until ${protocolDownloadGuarantee(purchase)}</p>
        </div>
        <span class="status">${statusText(status)}</span>
        <button class="text-button" data-download="${escapeAttr(purchase.txHash)}" type="button" ${canDownload && state.downloadBusy !== purchase.txHash ? "" : "disabled"}>
          ${state.downloadBusy === purchase.txHash ? "Working" : "Download"}
        </button>
        <button class="text-button" data-refund="${escapeAttr(purchase.txHash)}" title="${escapeAttr(refundState.reason)}" type="button" ${refundState.enabled && !refundBusy ? "" : "disabled"}>
          ${refundBusy ? "Refunding" : "Refund"}
        </button>
        <details class="record-details" data-record-details="${escapeAttr(detailId)}" ${state.expandedRecords.has(detailId) ? "open" : ""}>
          <summary>More information</summary>
          <dl class="record-facts">
            <div><dt>User key commitment</dt><dd><code>${escapeHtml(purchase.vssKeyCommitment)}</code></dd></div>
            <div><dt>User key</dt><dd><code>${secret?.userKey && visible.userKey ? escapeHtml(secret.userKey) : "Hidden"}</code> <button class="text-button compact" data-reveal-key="userKey" data-record-id="${escapeAttr(detailId)}" type="button">${visible.userKey ? "Hide" : "Show"}</button><small>ChaCha8 · ${escapeHtml(keyMode)}</small></dd></div>
            <div><dt>Data key commitment</dt><dd><code>${escapeHtml(state.dataKeyCommitments[detailId] ?? "Loading…")}</code></dd></div>
            <div><dt>Data key</dt><dd><code>${secret?.dataKey && visible.dataKey ? escapeHtml(secret.dataKey) : "Hidden"}</code> <button class="text-button compact" data-reveal-key="dataKey" data-record-id="${escapeAttr(detailId)}" type="button">${visible.dataKey ? "Hide" : "Show"}</button><small>32-byte key recovered from the fulfilled ChaCha8 key share</small></dd></div>
            <div><dt>Plaintext commitment</dt><dd><code>${escapeHtml(purchase.dataCommitment)}</code></dd></div>
            <div><dt>Ciphertext commitment / Walrus blob ID</dt><dd><code>${escapeHtml(proof?.cCipher ?? "Not published")}</code>${blobId ? `<small>${escapeHtml(blobId)} · <span class="walrus-${walrusState ?? "unchecked"}">${walrusLabel}</span></small>` : ""}</dd></div>
            <div><dt>Fulfill transaction</dt><dd>${share ? `<a href="https://sepolia.arbiscan.io/tx/${escapeAttr(share.txHash)}" target="_blank" rel="noreferrer"><code>${escapeHtml(share.txHash)}</code></a>` : "Not fulfilled"}</dd></div>
          </dl>
        </details>
      </article>
    `;
  });
  const localRows = state.localThreads
    .filter((thread) => !indexedTxHashes.has(thread.txHash.toLowerCase()))
    .map((thread) => {
      const sale = state.allSales.find((item) => item.channel.toLowerCase() === thread.channel.toLowerCase() && item.saleId.toLowerCase() === thread.saleId.toLowerCase());
      const detail = thread.lastError ? `${shortAddress(thread.txHash)} · ${thread.lastError}` : `${shortAddress(thread.txHash)} · local`;
      return `
      <article class="record">
        <span class="file-badge kind-${sale ? fileKind(sale.contentType, sale.fileName) : "binary"}">${escapeHtml(fileKindLabel(sale))}</span>
        <div class="record-main">
          <h2>${escapeHtml(thread.title)}</h2>
          <p>${escapeHtml(detail)}</p>
        </div>
        <span class="status">${statusText(thread.status)}</span>
        <button class="text-button" type="button" disabled>Download</button>
      </article>
    `;
    });
  return [...indexedRows, ...localRows].join("");
}

function dataKeyShareForPurchase(purchase: MarketplacePurchase): DataKeyShare | undefined {
  return [...state.dataKeyShares]
    .filter((share) => share.channel.toLowerCase() === purchase.channel.toLowerCase())
    .filter((share) => share.audiences.some((audience) => audience.toLowerCase() === purchase.buyer.toLowerCase()))
    .filter((share) => Number(share.timestamp) >= Number(purchase.timestamp))
    .sort((a, b) => Number(a.timestamp) - Number(b.timestamp))[0];
}

function vddProofForPurchase(purchase: MarketplacePurchase): VddProof | undefined {
  return [...state.vddProofs]
    .filter((proof) => proof.channel.toLowerCase() === purchase.channel.toLowerCase())
    .sort((a, b) => Number(b.timestamp) - Number(a.timestamp))[0];
}

function render(): void {
  const root = document.querySelector<HTMLDivElement>("#app");
  if (!root) return;

  if (state.route === "dataset-viewer") {
    root.innerHTML = `<iframe class="dataset-viewer-frame" src="/chain-intelligence-viewer/index.html" title="Chain Intelligence Dataset Viewer"></iframe>`;
  } else if (state.route === "browse") {
    root.innerHTML = renderBrowse();
  } else if (state.route === "records") {
    root.innerHTML = renderRecords();
  } else if (state.route === "settings") {
    root.innerHTML = renderSettings();
  } else if (state.route === "certificate") {
    root.innerHTML = renderCertificate();
  } else if (state.route === "dataset-certificate") {
    root.innerHTML = renderDatasetCertificate();
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
      if (state.route !== "certificate" && state.route !== "dataset-certificate") clearProofUrl();
      state.message = "";
      state.walletMenuOpen = false;
      render();
    });
  });

  root.querySelectorAll<HTMLElement>("[data-asset]").forEach((item) => {
    item.addEventListener("click", () => {
      state.selectedSaleId = item.dataset.asset ?? state.sales[0]?.id ?? "";
      state.route = "detail";
      state.message = "";
      state.walletMenuOpen = false;
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

  root.querySelector<HTMLButtonElement>("#load-more-sales-button")?.addEventListener("click", () => {
    void loadMoreMarketplaceSales();
  });
  root.querySelector<HTMLButtonElement>("#load-more-recommended-button")?.addEventListener("click", () => {
    void loadMoreRecommendedSales();
  });
  root.querySelector<HTMLButtonElement>("#download-certificate-button")?.addEventListener("click", () => {
    if (!state.videoProof) return;
    const bytes = JSON.stringify(state.videoProof.certificate, null, 2);
    const url = URL.createObjectURL(new Blob([`${bytes}\n`], { type: "application/json" }));
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = `trustdrop-video-certificate-${state.videoProof.certificateCid}.json`;
    anchor.click();
    URL.revokeObjectURL(url);
  });
  root.querySelector<HTMLButtonElement>("#download-dataset-certificate-button")?.addEventListener("click", () => {
    if (!state.datasetProof) return;
    const bytes = JSON.stringify(state.datasetProof.certificate, null, 2);
    const url = URL.createObjectURL(new Blob([`${bytes}\n`], { type: "application/json" }));
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = `trustdrop-chain-intelligence-v2-certificate-${state.datasetProof.certificateCid}.json`;
    anchor.click();
    URL.revokeObjectURL(url);
  });
  root.querySelector<HTMLButtonElement>("#copy-verification-command-button")?.addEventListener("click", async (event) => {
    if (!state.videoProof) return;
    const button = event.currentTarget as HTMLButtonElement;
    try {
      await navigator.clipboard.writeText(videoProofCurlCommand(state.videoProof.certificate));
      button.textContent = "Copied";
    } catch {
      button.textContent = "Copy failed";
    }
  });
  root.querySelectorAll<HTMLButtonElement>("[data-copy-dataset-verification]").forEach((button) => {
    button.addEventListener("click", async () => {
      if (!state.datasetProof) return;
      const kind = button.dataset.copyDatasetVerification as "flow" | "clusters";
      try { await navigator.clipboard.writeText(datasetSampleCurlCommand(kind, state.datasetProof.certificate)); button.textContent = "Copied"; }
      catch { button.textContent = "Copy failed"; }
    });
  });

  root.querySelector<HTMLButtonElement>("#wallet-button")?.addEventListener("click", () => {
    if (state.wallet) {
      state.walletMenuOpen = !state.walletMenuOpen;
      render();
    } else {
      void connect();
    }
  });
  root.querySelector<HTMLButtonElement>("#copy-wallet-button")?.addEventListener("click", () => {
    void copyWalletAddress();
  });
  root.querySelector<HTMLButtonElement>("#switch-wallet-button")?.addEventListener("click", () => {
    void switchAccount();
  });
  root.querySelector<HTMLButtonElement>("#disconnect-wallet-button")?.addEventListener("click", () => {
    disconnect();
  });
  root.querySelector<HTMLButtonElement>("#refresh-button")?.addEventListener("click", () => {
    void refreshMarketplaceAndRender();
  });
  for (const details of root.querySelectorAll<HTMLDetailsElement>("[data-record-details]")) {
    details.addEventListener("toggle", () => {
      const id = details.dataset.recordDetails;
      if (!id) return;
      if (details.open) {
        state.expandedRecords.add(id);
        void loadRecordDetails(id);
      } else {
        state.expandedRecords.delete(id);
      }
    });
  }
  for (const button of root.querySelectorAll<HTMLButtonElement>("[data-reveal-key]")) {
    button.addEventListener("click", (event) => {
      event.preventDefault();
      event.stopPropagation();
      const id = button.dataset.recordId;
      const key = button.dataset.revealKey as "userKey" | "dataKey";
      if (id) void toggleRecordKey(id, key);
    });
  }
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
  root.querySelectorAll<HTMLButtonElement>("[data-refund]").forEach((button) => {
    button.addEventListener("click", () => {
      void handleRefund(button.dataset.refund as `0x${string}`);
    });
  });
}

async function loadRecordDetails(id: string): Promise<void> {
  const purchase = state.purchases.find((item) => item.txHash.toLowerCase() === id);
  if (!purchase) return;
  const tasks: Promise<void>[] = [];
  if (!state.dataKeyCommitments[id]) {
    state.dataKeyCommitments[id] = "Loading…";
    tasks.push(readDataKeyCommitment(purchase.channel).then((value) => {
      state.dataKeyCommitments[id] = value;
    }).catch(() => {
      state.dataKeyCommitments[id] = "Unavailable";
    }));
  }
  const sale = findSaleForPurchase(purchase);
  const proof = vddProofForPurchase(purchase);
  if (sale && proof && !state.walrusChecks[id]) {
    state.walrusChecks[id] = "checking";
    tasks.push(isWalrusBlobReadable(state.aggregatorUrl, encryptedWalrusBlobId(sale, [proof])).then((readable) => {
      state.walrusChecks[id] = readable ? "readable" : "unavailable";
    }).catch(() => {
      state.walrusChecks[id] = "unavailable";
    }));
  }
  if (tasks.length === 0) return;
  await Promise.all(tasks);
  render();
}

async function toggleRecordKey(id: string, key: "userKey" | "dataKey"): Promise<void> {
  const visible = state.visibleRecordSecrets[id] ?? { userKey: false, dataKey: false };
  if (visible[key]) {
    state.visibleRecordSecrets[id] = { ...visible, [key]: false };
    render();
    return;
  }
  if (state.recordSecrets[id]?.[key]) {
    state.visibleRecordSecrets[id] = { ...visible, [key]: true };
    render();
    return;
  }
  const purchase = state.purchases.find((item) => item.txHash.toLowerCase() === id);
  if (!purchase || !state.wallet) return;
  if (purchase.buyer.toLowerCase() !== state.wallet.account.toLowerCase()) {
    state.message = "Connected wallet does not match this purchase.";
    render();
    return;
  }
  const sale = await saleForPurchase(purchase);
  const thread = state.localThreads.find((item) => item.txHash.toLowerCase() === id);
  const manualSecret = thread?.keyMode === "manual_secret" ? promptManualSecret("User key hex") : undefined;
  try {
    if (key === "userKey") {
      const userKey = manualSecret ?? `0x${Array.from(await deriveBuyerSecret(sale, state.wallet.account, state.wallet.client), (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
      state.recordSecrets[id] = { ...state.recordSecrets[id], userKey };
    } else {
      state.recordSecrets[id] = await recoverBuyerRecordKeys({
        sale,
        purchase,
        settlements: state.settlements,
        dataKeyShares: state.dataKeyShares,
        vddProofs: state.vddProofs,
        buyer: state.wallet.account,
        walletClient: state.wallet.client,
        manualSecret: manualSecret as `0x${string}` | undefined,
      });
    }
    state.visibleRecordSecrets[id] = { ...visible, [key]: true };
    state.message = "Keys are visible only in this page session.";
  } catch (error) {
    state.message = errorMessage(error);
  }
  render();
}

async function connect(): Promise<void> {
  try {
    state.wallet = await connectWallet();
    state.walletMenuOpen = false;
    await refreshBuyerActivity();
    state.message = "";
  } catch (error) {
    state.message = errorMessage(error);
  }
  render();
}

async function switchAccount(): Promise<void> {
  state.walletMenuOpen = false;
  await connect();
}

function disconnect(): void {
  state.wallet = null;
  state.walletMenuOpen = false;
  clearBuyerState();
  state.message = "Wallet disconnected.";
  render();
}

function clearBuyerState(): void {
  state.purchases = [];
  state.settlements = [];
  state.refunds = [];
  state.dataKeyShares = [];
  state.vddProofs = [];
  state.localThreads = [];
  state.purchaseBusy = false;
  state.downloadBusy = "";
  state.refundBusy = "";
}

function installWalletListeners(): void {
  onWalletAccountsChanged((accounts) => {
    void handleWalletAccountsChanged(accounts);
  });
  onWalletChainChanged(() => {
    if (!state.wallet) return;
    disconnect();
    state.message = "Wallet network changed. Reconnect on Arbitrum Sepolia.";
    render();
  });
}

async function handleWalletAccountsChanged(accounts: `0x${string}`[]): Promise<void> {
  const account = accounts[0];
  if (!account) {
    disconnect();
    return;
  }
  if (state.wallet?.account.toLowerCase() === account.toLowerCase()) return;
  try {
    clearBuyerState();
    state.wallet = await walletFromAccount(account);
    state.walletMenuOpen = false;
    await refreshBuyerActivity();
    state.message = "";
  } catch (error) {
    clearBuyerState();
    state.wallet = null;
    state.walletMenuOpen = false;
    state.message = errorMessage(error);
  }
  render();
}

async function copyWalletAddress(): Promise<void> {
  if (!state.wallet) return;
  try {
    await navigator.clipboard.writeText(state.wallet.account);
    state.message = "Wallet address copied.";
  } catch (error) {
    state.message = errorMessage(error);
  }
  state.walletMenuOpen = false;
  render();
}

async function handlePurchase(): Promise<void> {
  const sale = selectedSale();
  if (!sale) return;
  if (!state.wallet) {
    await connect();
    return;
  }
  if (state.keyMode === "manual_secret" && !ALLOW_MANUAL_SECRET) {
    state.keyMode = "wallet_derived";
  }
  state.purchaseBusy = true;
  state.message = "";
  render();
  try {
    const manualSecret = state.keyMode === "manual_secret" ? promptManualSecret("Purchase secret hex") : undefined;
    const prepared = await preparePurchase(sale, state.wallet.account, state.wallet.client, {
      manualSecret,
    });
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
  const thread = state.localThreads.find((item) => item.txHash.toLowerCase() === txHash.toLowerCase());
  if (thread?.keyMode === "manual_secret" && !ALLOW_MANUAL_SECRET) {
    state.message = "Manual secret recovery is available only in development mode.";
    render();
    return;
  }
  const manualSecret = thread?.keyMode === "manual_secret" ? promptManualSecret("Recovery secret hex") : undefined;
  state.downloadBusy = txHash;
  state.message = "";
  render();
  try {
    const sale = await saleForPurchase(purchase);
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

async function saleForPurchase(purchase: MarketplacePurchase): Promise<MarketplaceSale> {
  const existing = findSaleForPurchase(purchase);
  if (existing) return existing;
  const sale = await subgraph.getSale(purchase.channel, purchase.saleId);
  if (!sale) throw new Error("Unable to load purchase recovery data. Refresh records and try again.");
  state.allSales = [sale, ...state.allSales.filter((item) => item.id.toLowerCase() !== sale.id.toLowerCase())];
  return sale;
}

async function handleRefund(txHash: `0x${string}`): Promise<void> {
  const purchase = state.purchases.find((item) => item.txHash.toLowerCase() === txHash.toLowerCase());
  if (!purchase || !state.wallet) return;
  state.refundBusy = txHash;
  state.message = "";
  render();
  try {
    const refundTx = await refundPurchase(purchase, state.wallet.client);
    state.message = `Refund submitted: ${shortAddress(refundTx)}`;
    await refreshBuyerActivity();
  } catch (error) {
    state.message = errorMessage(error);
  } finally {
    state.refundBusy = "";
    render();
  }
}

function promptManualSecret(label: string): `0x${string}` {
  const input = prompt(`${label} (32 bytes hex)`);
  if (!input) throw new Error("Manual secret is required");
  return normalizeSecretHex(input, label);
}

function normalizeSecretHex(value: string, label: string): `0x${string}` {
  const hex = value.trim().toLowerCase().replace(/^0x/, "");
  if (!/^[0-9a-f]+$/.test(hex)) throw new Error(`${label} must be hex`);
  if (hex.length !== 64) throw new Error(`${label} must be exactly 32 bytes`);
  return `0x${hex}`;
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

function contentRulesUnavailable(): string {
  return `
    <section class="toolbar compact">
      <div>
        <h1>Marketplace unavailable</h1>
        <p>Content rules must load before listings can be shown.</p>
      </div>
    </section>
    ${state.message ? `<div class="notice">${escapeHtml(state.message)}</div>` : ""}
  `;
}

function contentRulesLoading(): string {
  return `
    <section class="loading-screen" aria-live="polite">
      <div class="loading-spinner" aria-hidden="true"></div>
      <div>
        <h1>Loading marketplace</h1>
        <p>Fetching content rules and marketplace listings.</p>
      </div>
    </section>
  `;
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
  return new Date(timestamp * 1000).toLocaleString(undefined, {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}

function protocolDownloadGuarantee(purchase: MarketplacePurchase): string {
  const initTime = Number(purchase.initTime);
  if (!Number.isFinite(initTime) || initTime <= 0) return "-";
  return formatTimestamp(String(initTime + 7 * 24 * 60 * 60));
}

function refundAvailability(
  purchase: MarketplacePurchase,
  status: ReturnType<typeof buyerAssetStatus> | "refunded",
): { enabled: boolean; reason: string } {
  if (!state.wallet) return { enabled: false, reason: "Connect wallet to refund" };
  if (status === "ready_to_download") return { enabled: false, reason: "Settled purchases cannot be refunded" };
  if (status === "refunded") return { enabled: false, reason: "Purchase already refunded" };
  const deadline = Number(purchase.deadline);
  if (!Number.isFinite(deadline)) return { enabled: false, reason: "Invalid refund deadline" };
  if (deadline >= currentUnixTime()) return { enabled: false, reason: `Refund available after ${formatTimestamp(purchase.deadline)}` };
  return { enabled: true, reason: "Refund expired pending purchase" };
}

function currentUnixTime(): number {
  return Math.floor(Date.now() / 1000);
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
