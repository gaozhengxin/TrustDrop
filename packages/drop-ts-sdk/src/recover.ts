import { sha256 } from "@noble/hashes/sha2.js";
import { concatBytes, hexToBytes, utf8ToBytes } from "@noble/hashes/utils.js";
import { keccak256, type WalletClient } from "viem";
import { arbitrumSepolia } from "viem/chains";
import type { DataKeyShare, MarketplacePurchase, MarketplaceSale, MarketplaceSettlement, VddProof } from "./subgraph";
import { downloadWalrusBlob, walrusBlobIdFromHex } from "./walrus";

type Hex = `0x${string}`;

export type RecoveryKeyMode = "wallet_derived" | "manual_secret";

export type RecoverAssetInput = {
  sale: MarketplaceSale;
  purchase: MarketplacePurchase;
  settlements: MarketplaceSettlement[];
  dataKeyShares: DataKeyShare[];
  vddProofs: VddProof[];
  buyer: Hex;
  walletClient: WalletClient;
  aggregatorUrl: string;
  manualSecret?: Hex;
};

export type RecoverAssetResult = {
  bytes: Uint8Array;
  fileName: string;
  contentType: string;
};

export async function recoverPurchasedAsset(input: RecoverAssetInput): Promise<RecoverAssetResult> {
  validateSaleDataCommitment(input.sale);
  if (!sameHex(input.sale.dataCommitment, input.purchase.dataCommitment)) {
    throw new Error("Purchase data commitment does not match sale data commitment");
  }
  const share = findDataKeyShare(input.dataKeyShares, input.buyer, input.purchase.timestamp);
  if (!share) throw new Error("No data key share found for this purchase");
  const encryptedDataKey = encryptedKeyForAudience(share, input.buyer);
  const secret = input.manualSecret
    ? bytesFromHex(input.manualSecret, "manual recovery secret", 32)
    : await deriveBuyerSecret(input.sale, input.buyer, input.walletClient);
  const assetKey = chacha8Xor(encryptedDataKey, secret, new Uint8Array(12), 0);
  const blobId = walrusBlobId(input.sale, input.vddProofs);
  const encrypted = await downloadWalrusBlob(input.aggregatorUrl, blobId);
  const nonce = deriveRslhNonce(assetKey, utf8ToBytes("trustdrop_asset_v1"));
  const paddedPlaintext = chacha8Xor(encrypted, assetKey, nonce, 0);
  const originalSize = Number(input.sale.fileSize);
  const bytes = Number.isFinite(originalSize) && originalSize > 0 ? paddedPlaintext.slice(0, originalSize) : paddedPlaintext;
  return {
    bytes,
    fileName: input.sale.fileName || `${input.sale.saleId}.bin`,
    contentType: input.sale.contentType || "application/octet-stream",
  };
}

export async function deriveBuyerSecret(
  sale: MarketplaceSale,
  buyer: Hex,
  walletClient: WalletClient,
): Promise<Uint8Array> {
  const signature = await walletClient.signMessage({
    account: buyer,
    message: [
      "Fair File Marketplace buyer key",
      `chain:${arbitrumSepolia.id}`,
      `channel:${sale.channel.toLowerCase()}`,
      `sale:${sale.saleId.toLowerCase()}`,
      `version:${sale.version.toLowerCase()}`,
      `commitment:${sale.dataCommitment.toLowerCase()}`,
    ].join("\n"),
  });
  return sha256(concatBytes(bytesFromHex(signature, "wallet signature"), utf8ToBytes(sale.saleId)));
}

export function buyerAssetStatus(
  purchase: MarketplacePurchase,
  settlements: MarketplaceSettlement[],
  dataKeyShares: DataKeyShare[],
): "refunded" | "ready_to_download" | "fulfilled" | "waiting_fulfill" {
  const settled = settlements.some(
    (settlement) =>
      sameHex(settlement.channel, purchase.channel) &&
      sameHex(settlement.saleId, purchase.saleId) &&
      sameHex(settlement.buyer, purchase.buyer) &&
      Number(settlement.timestamp) >= Number(purchase.timestamp),
  );
  if (settled) return "ready_to_download";
  const fulfilled = dataKeyShares.some(
    (share) =>
      sameHex(share.channel, purchase.channel) &&
      share.audiences.some((audience) => sameHex(audience, purchase.buyer)) &&
      Number(share.timestamp) >= Number(purchase.timestamp),
  );
  return fulfilled ? "fulfilled" : "waiting_fulfill";
}

export function fileKind(contentType: string, fileName: string): "data" | "text" | "image" | "video" | "audio" | "program" | "binary" {
  const type = contentType.toLowerCase();
  const name = fileName.toLowerCase();
  if (type.startsWith("text/") || /\.(txt|md|csv|json|xml|log)$/.test(name)) return "text";
  if (type.startsWith("image/") || /\.(png|jpe?g|gif|webp|svg)$/.test(name)) return "image";
  if (type.startsWith("video/") || /\.(mp4|mov|mkv|webm|avi)$/.test(name)) return "video";
  if (type.startsWith("audio/") || /\.(mp3|wav|flac|aac|ogg)$/.test(name)) return "audio";
  if (/\.(exe|dmg|pkg|deb|rpm|appimage|bin|wasm)$/.test(name)) return "program";
  if (type.includes("json") || type.includes("parquet") || type.includes("database")) return "data";
  return "binary";
}

function findDataKeyShare(dataKeyShares: DataKeyShare[], buyer: Hex, purchaseTimestamp: string): DataKeyShare | null {
  const minTimestamp = Number(purchaseTimestamp);
  return (
    [...dataKeyShares]
      .filter((share) => share.audiences.some((audience) => sameHex(audience, buyer)))
      .filter((share) => Number(share.timestamp) >= minTimestamp)
      .sort((a, b) => Number(a.timestamp) - Number(b.timestamp))[0] ?? null
  );
}

function encryptedKeyForAudience(share: DataKeyShare, buyer: Hex): Uint8Array {
  const index = share.audiences.findIndex((audience) => sameHex(audience, buyer));
  if (index < 0) throw new Error("Buyer is not in data key share audience");
  const encrypted = share.encryptedDataKeys[index];
  if (!encrypted) throw new Error("Missing encrypted data key for buyer");
  return bytesFromHex(encrypted, "encrypted data key", 32);
}

function walrusBlobId(sale: MarketplaceSale, vddProofs: VddProof[]): string {
  const parsed = parseSaleInfo(sale.info);
  const explicit = parsed.walrusBlobId || parsed.walrus_blob_id || parsed.blobId;
  if (typeof explicit === "string" && explicit.length > 0) return explicit;
  const proof = [...vddProofs]
    .filter((item) => sameHex(item.channel, sale.channel))
    .sort((a, b) => Number(b.timestamp) - Number(a.timestamp))[0];
  if (proof) return walrusBlobIdFromHex(proof.cCipher);
  throw new Error("Missing encrypted Walrus blob id; wait for VDD proof indexing or refresh activity");
}

function parseSaleInfo(info: string): Record<string, unknown> {
  try {
    const parsed = JSON.parse(info) as unknown;
    return parsed && typeof parsed === "object" && !Array.isArray(parsed) ? (parsed as Record<string, unknown>) : {};
  } catch {
    return {};
  }
}

function validateSaleDataCommitment(sale: MarketplaceSale): void {
  if (keccak256(sale.dataCommitment).toLowerCase() !== sale.version.toLowerCase()) {
    throw new Error("Sale data commitment does not match sale data version");
  }
}

function deriveRslhNonce(key: Uint8Array, auxData: Uint8Array): Uint8Array {
  return sha256(concatBytes(key, auxData)).slice(0, 12);
}

function chacha8Xor(input: Uint8Array, key: Uint8Array, nonce: Uint8Array, counter: number): Uint8Array {
  if (key.length !== 32) throw new Error("ChaCha8 key must be 32 bytes");
  if (nonce.length !== 12) throw new Error("ChaCha8 nonce must be 12 bytes");
  const output = new Uint8Array(input);
  const block = new Uint8Array(64);
  for (let offset = 0, blockCounter = counter; offset < output.length; offset += 64, blockCounter++) {
    chachaBlock(key, nonce, blockCounter, block);
    for (let i = 0; i < 64 && offset + i < output.length; i++) output[offset + i] ^= block[i];
  }
  return output;
}

function chachaBlock(key: Uint8Array, nonce: Uint8Array, counter: number, out: Uint8Array): void {
  const state = new Uint32Array(16);
  state[0] = 0x61707865;
  state[1] = 0x3320646e;
  state[2] = 0x79622d32;
  state[3] = 0x6b206574;
  for (let i = 0; i < 8; i++) state[4 + i] = readU32(key, i * 4);
  state[12] = counter >>> 0;
  state[13] = readU32(nonce, 0);
  state[14] = readU32(nonce, 4);
  state[15] = readU32(nonce, 8);
  const working = new Uint32Array(state);
  for (let i = 0; i < 4; i++) {
    quarterRound(working, 0, 4, 8, 12);
    quarterRound(working, 1, 5, 9, 13);
    quarterRound(working, 2, 6, 10, 14);
    quarterRound(working, 3, 7, 11, 15);
    quarterRound(working, 0, 5, 10, 15);
    quarterRound(working, 1, 6, 11, 12);
    quarterRound(working, 2, 7, 8, 13);
    quarterRound(working, 3, 4, 9, 14);
  }
  for (let i = 0; i < 16; i++) writeU32(out, i * 4, (working[i] + state[i]) >>> 0);
}

function quarterRound(state: Uint32Array, a: number, b: number, c: number, d: number): void {
  state[a] = (state[a] + state[b]) >>> 0;
  state[d] = rotl(state[d] ^ state[a], 16);
  state[c] = (state[c] + state[d]) >>> 0;
  state[b] = rotl(state[b] ^ state[c], 12);
  state[a] = (state[a] + state[b]) >>> 0;
  state[d] = rotl(state[d] ^ state[a], 8);
  state[c] = (state[c] + state[d]) >>> 0;
  state[b] = rotl(state[b] ^ state[c], 7);
}

function rotl(value: number, bits: number): number {
  return ((value << bits) | (value >>> (32 - bits))) >>> 0;
}

function readU32(bytes: Uint8Array, offset: number): number {
  return (bytes[offset] | (bytes[offset + 1] << 8) | (bytes[offset + 2] << 16) | (bytes[offset + 3] << 24)) >>> 0;
}

function writeU32(bytes: Uint8Array, offset: number, value: number): void {
  bytes[offset] = value & 0xff;
  bytes[offset + 1] = (value >>> 8) & 0xff;
  bytes[offset + 2] = (value >>> 16) & 0xff;
  bytes[offset + 3] = (value >>> 24) & 0xff;
}

function bytesFromHex(value: string, label: string, expectedLength?: number): Uint8Array {
  const normalized = value.startsWith("0x") ? value.slice(2) : value;
  if (!/^[0-9a-fA-F]*$/.test(normalized) || normalized.length % 2 !== 0) throw new Error(`${label} must be even hex`);
  const bytes = hexToBytes(normalized);
  if (expectedLength !== undefined && bytes.length !== expectedLength) {
    throw new Error(`${label} must be ${expectedLength} bytes`);
  }
  return bytes;
}

function sameHex(a: string, b: string): boolean {
  return a.toLowerCase() === b.toLowerCase();
}
