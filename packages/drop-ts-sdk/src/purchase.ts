import { blake3 } from "@noble/hashes/blake3.js";
import { sha256 } from "@noble/hashes/sha2.js";
import { concatBytes, hexToBytes, utf8ToBytes } from "@noble/hashes/utils.js";
import { secp256k1 } from "@noble/curves/secp256k1.js";
import {
  createPublicClient,
  formatEther,
  http,
  parseAbi,
  type WalletClient,
} from "viem";
import { arbitrumSepolia } from "viem/chains";
import { DEFAULT_RPC_URL } from "./config";
import type { MarketplaceSale } from "./subgraph";

const channelAbi = parseAbi([
  "function ownerPublicKey() view returns ((bytes data))",
  "function purchase(bytes32 saleId, bytes32 dataVersion, uint256 price, uint256 deadline, bytes dataCommitment, bytes32 vssKeyCommitment, bytes32 encryptedVssKey, bytes ephemeralPubkey) payable",
]);

export type PreparedPurchase = {
  sale: MarketplaceSale;
  deadline: bigint;
  secretSharingKey: `0x${string}`;
  vssKeyCommitment: `0x${string}`;
  encryptedVssKey: `0x${string}`;
  ephemeralPubkey: `0x${string}`;
};

export function salePriceEth(sale: MarketplaceSale): string {
  return formatEther(BigInt(sale.price));
}

export function saleDisplayTitle(sale: MarketplaceSale): string {
  if (sale.title && sale.title !== "TrustDrop Asset v1") return sale.title;
  if (sale.fileName) return sale.fileName;
  return `File ${sale.saleId.slice(0, 10)}`;
}

export async function preparePurchase(
  sale: MarketplaceSale,
  buyer: `0x${string}`,
  walletClient: WalletClient,
): Promise<PreparedPurchase> {
  const signature = await walletClient.signMessage({
    account: buyer,
    message: [
      "Trusted File Marketplace buyer key",
      `chain:${arbitrumSepolia.id}`,
      `channel:${sale.channel.toLowerCase()}`,
      `sale:${sale.saleId.toLowerCase()}`,
      `version:${sale.version.toLowerCase()}`,
      `commitment:${sale.dataCommitment.toLowerCase()}`,
    ].join("\n"),
  });
  const secretBytes = sha256(concatBytes(hexToBytes(signature), utf8ToBytes(sale.saleId)));
  const commitment = blake3(secretBytes);

  const publicClient = createPublicClient({
    chain: arbitrumSepolia,
    transport: http(DEFAULT_RPC_URL),
  });
  const ownerPublicKey = (await publicClient.readContract({
    address: sale.channel,
    abi: channelAbi,
    functionName: "ownerPublicKey",
  })) as { data: `0x${string}` } | [`0x${string}`];
  const ownerPublicKeyHex = Array.isArray(ownerPublicKey) ? ownerPublicKey[0] : ownerPublicKey.data;
  const encrypted = eciesEncrypt(ownerPublicKeyHex, secretBytes);

  return {
    sale,
    deadline: BigInt(Math.floor(Date.now() / 1000) + 8 * 24 * 60 * 60),
    secretSharingKey: bytesToHex(secretBytes),
    vssKeyCommitment: bytesToHex(commitment),
    encryptedVssKey: bytesToHex(encrypted.ciphertext),
    ephemeralPubkey: bytesToHex(encrypted.ephemeralPubkey),
  };
}

export async function submitPurchase(prepared: PreparedPurchase, walletClient: WalletClient): Promise<`0x${string}`> {
  if (!walletClient.account) throw new Error("Wallet account is required");
  return walletClient.writeContract({
    address: prepared.sale.channel,
    abi: channelAbi,
    functionName: "purchase",
    args: [
      prepared.sale.saleId,
      prepared.sale.version,
      BigInt(prepared.sale.price),
      prepared.deadline,
      prepared.sale.dataCommitment,
      prepared.vssKeyCommitment,
      prepared.encryptedVssKey,
      prepared.ephemeralPubkey,
    ],
    value: BigInt(prepared.sale.price),
    account: walletClient.account,
    chain: arbitrumSepolia,
  });
}

function eciesEncrypt(recipientPubkey: `0x${string}`, secret: Uint8Array): { ciphertext: Uint8Array; ephemeralPubkey: Uint8Array } {
  const ephemeralSecret = secp256k1.utils.randomSecretKey();
  const shared = secp256k1.getSharedSecret(ephemeralSecret, hexToBytes(recipientPubkey), false);
  const mask = sha256(shared.slice(1, 33));
  const ciphertext = new Uint8Array(32);
  for (let i = 0; i < 32; i++) ciphertext[i] = secret[i] ^ mask[i];
  return {
    ciphertext,
    ephemeralPubkey: secp256k1.getPublicKey(ephemeralSecret, true),
  };
}

function bytesToHex(bytes: Uint8Array): `0x${string}` {
  return `0x${Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
}
