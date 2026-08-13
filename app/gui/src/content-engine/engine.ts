import { createPublicClient, http, isAddress, parseAbi } from "viem";
import { arbitrumSepolia } from "viem/chains";
import { DEFAULT_RPC_URL, type MarketplaceSale } from "../../../../packages/drop-ts-sdk/src";

type Hex = `0x${string}`;
type ImportMetaWithEnv = ImportMeta & {
  env?: Record<string, string | undefined>;
};

export type VisionAssetRef = {
  channel: Hex;
  saleId: Hex;
  reason?: string;
};

export type VisionSellerRef = {
  seller: Hex;
  reason?: string;
};

export type VisionDescriptor = {
  schema: "trustdrop.vision.v1";
  network: "arbitrum-sepolia";
  version: number;
  updatedAt: string;
  source?: {
    type: string;
    cid?: string;
  };
  recommendations?: {
    featuredAssets?: VisionAssetRef[];
  };
  moderation?: {
    startTimestamp?: number;
    minimumListedBlock?: string;
    blacklistedAssets?: VisionAssetRef[];
    blacklistedSellers?: VisionSellerRef[];
  };
};

export type HiddenReason = "before_start_timestamp" | "before_minimum_block" | "asset_blacklisted" | "seller_blacklisted";

const visionRegistryAbi = parseAbi(["function visionCid() view returns (string)"]);
const DEFAULT_VISION_REGISTRY_ADDRESS = "0x79A070bF4b64f815249F4ac0ea05bdB983b92261";
const DEFAULT_IPFS_GATEWAY = "https://ipfs.io/ipfs/";
const PUBLIC_IPFS_GATEWAYS = ["https://gateway.pinata.cloud/ipfs/", "https://dweb.link/ipfs/", "https://nftstorage.link/ipfs/"];
const VISION_FETCH_TIMEOUT_MS = 8_000;

let activeVision: VisionDescriptor | null = null;

export async function loadVisionDescriptor(): Promise<VisionDescriptor> {
  const registryAddress = visionRegistryAddress();
  if (!registryAddress) {
    throw new Error("Vision registry address is not configured");
  }
  const client = createPublicClient({
    chain: arbitrumSepolia,
    transport: http(DEFAULT_RPC_URL),
  });
  const cid = (await client.readContract({
    address: registryAddress,
    abi: visionRegistryAbi,
    functionName: "visionCid",
  })) as string;
  if (!cid) throw new Error("Vision registry returned empty CID");
  const descriptor = await fetchVision(cid);
  activeVision = validateVision({
    ...descriptor,
    source: { type: descriptor.source?.type ?? "ipfs", cid },
  });
  return activeVision;
}

export function activeContentRule(): VisionDescriptor {
  if (!activeVision) throw new Error("Vision descriptor is not loaded");
  return activeVision;
}

export function featuredAssetRefs(): VisionAssetRef[] {
  if (!activeVision) throw new Error("Vision descriptor is not loaded");
  return activeVision.recommendations?.featuredAssets ?? [];
}

export function marketplaceQueryBounds(): { minimumListedTimestamp?: string; minimumListedBlock?: string } {
  if (!activeVision) throw new Error("Vision descriptor is not loaded");
  const moderation = activeVision.moderation ?? {};
  return {
    minimumListedTimestamp: Number.isFinite(moderation.startTimestamp) ? String(moderation.startTimestamp) : undefined,
    minimumListedBlock: moderation.minimumListedBlock,
  };
}

export function filterSalesForContentEngine(sales: MarketplaceSale[]): MarketplaceSale[] {
  return sales.filter((sale) => hiddenReasonsForSale(sale).length === 0);
}

export function hiddenReasonsForSale(sale: MarketplaceSale): HiddenReason[] {
  if (!activeVision) throw new Error("Vision descriptor is not loaded");
  const reasons: HiddenReason[] = [];
  const moderation = activeVision.moderation ?? {};
  const listedAt = Number(sale.listedAtTimestamp);
  if (moderation.startTimestamp && Number.isFinite(listedAt) && listedAt < moderation.startTimestamp) {
    reasons.push("before_start_timestamp");
  }
  if (moderation.minimumListedBlock && parseBigInt(sale.listedAtBlock) < parseBigInt(moderation.minimumListedBlock)) {
    reasons.push("before_minimum_block");
  }
  if ((moderation.blacklistedAssets ?? []).some((asset) => sameAsset(asset, sale))) {
    reasons.push("asset_blacklisted");
  }
  if ((moderation.blacklistedSellers ?? []).some((seller) => sameAddress(seller.seller, sale.channel))) {
    reasons.push("seller_blacklisted");
  }
  return reasons;
}

async function fetchVision(cid: string): Promise<VisionDescriptor> {
  const cleanCid = cid.replace(/^ipfs:\/\//, "");
  const env = (import.meta as ImportMetaWithEnv).env ?? {};
  const gateway = (env.VITE_TRUSTDROP_IPFS_GATEWAY || DEFAULT_IPFS_GATEWAY).replace(/\/?$/, "/");
  const urls = Array.from(new Set([gateway, ...PUBLIC_IPFS_GATEWAYS].map((item) => item.replace(/\/?$/, "/")))).map((item) => `${item}${cleanCid}`);
  return firstSuccessfulVision(urls);
}

async function firstSuccessfulVision(urls: string[]): Promise<VisionDescriptor> {
  const failures: string[] = [];
  return new Promise((resolve, reject) => {
    let pending = urls.length;
    for (const url of urls) {
      fetchVisionUrl(url)
        .then(resolve)
        .catch((error) => {
          failures.push(`${url}: ${errorMessage(error)}`);
          pending -= 1;
          if (pending === 0) reject(new Error(`Vision fetch failed from all gateways: ${failures.join("; ")}`));
        });
    }
  });
}

async function fetchVisionUrl(url: string): Promise<VisionDescriptor> {
  const controller = new AbortController();
  const timeout = window.setTimeout(() => controller.abort(), VISION_FETCH_TIMEOUT_MS);
  try {
    const response = await fetch(url, {
      headers: { accept: "application/json" },
      signal: controller.signal,
    });
    if (!response.ok) throw new Error(`Vision fetch failed: HTTP ${response.status}`);
    return (await response.json()) as VisionDescriptor;
  } finally {
    window.clearTimeout(timeout);
  }
}

function validateVision(value: VisionDescriptor): VisionDescriptor {
  if (value.schema !== "trustdrop.vision.v1") throw new Error("Unsupported vision schema");
  if (value.network !== "arbitrum-sepolia") throw new Error("Unsupported vision network");
  if (!Number.isFinite(value.version)) throw new Error("Invalid vision version");
  return value;
}

function visionRegistryAddress(): Hex | null {
  const raw = (import.meta as ImportMetaWithEnv).env?.VITE_TRUSTDROP_VISION_REGISTRY_ADDRESS ?? DEFAULT_VISION_REGISTRY_ADDRESS;
  return isAddress(raw) ? raw : null;
}

function sameAsset(asset: VisionAssetRef, sale: MarketplaceSale): boolean {
  return assetKey(asset.channel, asset.saleId) === saleKey(sale);
}

function saleKey(sale: MarketplaceSale): string {
  return assetKey(sale.channel, sale.saleId);
}

function assetKey(channel: string, saleId: string): string {
  return `${channel}:${saleId}`.toLowerCase();
}

function sameAddress(left: string, right: string): boolean {
  return left.toLowerCase() === right.toLowerCase();
}

function parseBigInt(value: string): bigint {
  try {
    return BigInt(value);
  } catch {
    return 0n;
  }
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
