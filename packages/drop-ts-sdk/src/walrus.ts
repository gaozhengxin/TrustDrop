export type WalrusAggregatorPreset = {
  id: string;
  name: string;
  url: string;
};

export const WALRUS_AGGREGATOR_PRESETS: WalrusAggregatorPreset[] = [
  {
    id: "mysten-mainnet",
    name: "Mysten Labs",
    url: "https://aggregator.walrus-mainnet.walrus.space",
  },
  {
    id: "h2o-mainnet",
    name: "H2O Nodes",
    url: "https://aggregator.walrus-mainnet.h2o-nodes.com",
  },
  {
    id: "mirai-mainnet",
    name: "Studio Mirai",
    url: "https://aggregator.mainnet.walrus.mirai.cloud",
  },
];

const STORE_KEY = "ffm_walrus_aggregator_url";

export function defaultWalrusAggregatorUrl(): string {
  return WALRUS_AGGREGATOR_PRESETS[0].url;
}

export function normalizeAggregatorUrl(value: string): string {
  const trimmed = value.trim();
  if (!trimmed) throw new Error("Aggregator URL is required");
  const url = new URL(trimmed);
  if (url.protocol !== "https:" && url.protocol !== "http:") {
    throw new Error("Aggregator URL must use http or https");
  }
  url.pathname = url.pathname.replace(/\/+$/, "");
  url.search = "";
  url.hash = "";
  return url.toString().replace(/\/$/, "");
}

export function getStoredWalrusAggregatorUrl(): string {
  const stored = globalThis.localStorage?.getItem(STORE_KEY);
  if (!stored) return defaultWalrusAggregatorUrl();
  try {
    return normalizeAggregatorUrl(stored);
  } catch {
    return defaultWalrusAggregatorUrl();
  }
}

export function setStoredWalrusAggregatorUrl(value: string): string {
  const normalized = normalizeAggregatorUrl(value);
  globalThis.localStorage?.setItem(STORE_KEY, normalized);
  return normalized;
}

export async function checkWalrusAggregator(url: string): Promise<void> {
  const normalized = normalizeAggregatorUrl(url);
  const response = await fetch(`${normalized}/v1/api`, { method: "GET" });
  if (!response.ok) {
    throw new Error(`Aggregator unavailable: HTTP ${response.status}`);
  }
}

export async function downloadWalrusBlob(url: string, blobId: string): Promise<Uint8Array> {
  const normalized = normalizeAggregatorUrl(url);
  const response = await fetch(`${normalized}/v1/blobs/${encodeURIComponent(blobId)}`);
  if (!response.ok) {
    throw new Error(`Walrus blob download failed: HTTP ${response.status}`);
  }
  return new Uint8Array(await response.arrayBuffer());
}

export function walrusBlobIdFromHex(hex: `0x${string}`): string {
  const bytes = hexToBytes32(hex, "Walrus blob id bytes");
  return base64UrlNoPad(bytes);
}

function hexToBytes32(value: string, label: string): Uint8Array {
  const hex = value.startsWith("0x") ? value.slice(2) : value;
  if (!/^[0-9a-fA-F]{64}$/.test(hex)) throw new Error(`${label} must be 32 bytes hex`);
  const out = new Uint8Array(32);
  for (let i = 0; i < out.length; i++) out[i] = Number.parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  return out;
}

function base64UrlNoPad(bytes: Uint8Array): string {
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/g, "");
}
