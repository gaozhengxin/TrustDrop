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

type Hex = `0x${string}`;

const channelAbi = parseAbi([
  "function ownerPublicKey() view returns (bytes data)",
  "function isRegistered(address user) view returns (bool)",
  "function purchase(bytes32 saleId, bytes32 dataVersion, uint256 price, uint256 deadline, bytes dataCommitment, bytes32 vssKeyCommitment, bytes encryptedVssKey) payable",
]);

const ZERO_HASH = "0x0000000000000000000000000000000000000000000000000000000000000000";

export type PreparedPurchase = {
  sale: MarketplaceSale;
  deadline: bigint;
  secretSharingKey: `0x${string}`;
  vssKeyCommitment: `0x${string}`;
  encryptedVssKey: `0x${string}`;
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
      "Fair File Marketplace buyer key",
      `chain:${arbitrumSepolia.id}`,
      `channel:${sale.channel.toLowerCase()}`,
      `sale:${sale.saleId.toLowerCase()}`,
      `version:${sale.version.toLowerCase()}`,
      `commitment:${sale.dataCommitment.toLowerCase()}`,
    ].join("\n"),
  });
  const secretBytes = sha256(concatBytes(bytesFromHex(signature, "wallet signature"), utf8ToBytes(sale.saleId)));
  const commitment = blake3(secretBytes);
  const publicClient = createPublicClient({
    chain: arbitrumSepolia,
    transport: http(DEFAULT_RPC_URL),
  });
  const registered = (await publicClient.readContract({
    address: sale.channel,
    abi: channelAbi,
    functionName: "isRegistered",
    args: [buyer],
  })) as boolean;
  if (registered) {
    return {
      sale,
      deadline: BigInt(Math.floor(Date.now() / 1000) + 8 * 24 * 60 * 60),
      secretSharingKey: "0x",
      vssKeyCommitment: ZERO_HASH,
      encryptedVssKey: "0x",
    };
  }

  const ownerPublicKey = (await publicClient.readContract({
    address: sale.channel,
    abi: channelAbi,
    functionName: "ownerPublicKey",
  })) as `0x${string}`;
  const ownerPublicKeyHex = normalizeHex(ownerPublicKey, "seller owner public key");
  const encryptedVssKey = eciesEncryptPackage(ownerPublicKeyHex, secretBytes);

  return {
    sale,
    deadline: BigInt(Math.floor(Date.now() / 1000) + 8 * 24 * 60 * 60),
    secretSharingKey: bytesToHex(secretBytes),
    vssKeyCommitment: bytesToHex(commitment),
    encryptedVssKey: bytesToHex(encryptedVssKey),
  };
}

export async function submitPurchase(prepared: PreparedPurchase, walletClient: WalletClient): Promise<`0x${string}`> {
  if (!walletClient.account) throw new Error("Wallet account is required");
  const account = walletClient.account.address;
  const args = [
    prepared.sale.saleId,
    prepared.sale.version,
    BigInt(prepared.sale.price),
    prepared.deadline,
    prepared.sale.dataCommitment,
    prepared.vssKeyCommitment,
    prepared.encryptedVssKey,
  ] as const;
  const publicClient = createPublicClient({
    chain: arbitrumSepolia,
    transport: http(DEFAULT_RPC_URL),
  });
  const estimatedGas = await publicClient.estimateContractGas({
    address: prepared.sale.channel,
    abi: channelAbi,
    functionName: "purchase",
    args,
    value: BigInt(prepared.sale.price),
    account,
  });
  const latestBlock = await publicClient.getBlock();
  const baseFee = latestBlock.baseFeePerGas ?? 20_000_000n;
  const maxPriorityFeePerGas = 1_000_000n;
  const maxFeePerGas = baseFee * 2n + maxPriorityFeePerGas;

  const request = {
    address: prepared.sale.channel,
    abi: channelAbi,
    functionName: "purchase",
    args,
    value: BigInt(prepared.sale.price),
    gas: estimatedGas + estimatedGas / 5n,
    maxFeePerGas,
    maxPriorityFeePerGas,
    account: walletClient.account,
    chain: arbitrumSepolia,
  } as const;

  try {
    return await walletClient.writeContract(request);
  } catch (error) {
    if (!isNonceTooLowError(error)) throw error;
    const nonce = await publicClient.getTransactionCount({
      address: account,
      blockTag: "pending",
    });
    return walletClient.writeContract({
      ...request,
      nonce,
    });
  }
}

function eciesEncryptPackage(recipientPubkey: Hex, secret: Uint8Array): Uint8Array {
  const version = 1;
  const recipientPubkeyBytes = bytesFromHex(recipientPubkey, "seller owner public key");
  if (recipientPubkeyBytes.length !== 33 && recipientPubkeyBytes.length !== 65) {
    throw new Error(`Invalid seller owner public key length: ${recipientPubkeyBytes.length} bytes`);
  }
  const ephemeralSecret = secp256k1.utils.randomSecretKey();
  const shared = secp256k1.getSharedSecret(ephemeralSecret, recipientPubkeyBytes, false);
  const mask = sha256(shared.slice(1, 33));
  const ciphertext = new Uint8Array(32);
  for (let i = 0; i < 32; i++) ciphertext[i] = secret[i] ^ mask[i];
  return concatBytes(new Uint8Array([version]), secp256k1.getPublicKey(ephemeralSecret, true), ciphertext);
}

function normalizeHex(value: string, label: string): Hex {
  let hex = value.trim().toLowerCase();
  if (hex.startsWith("0x")) hex = hex.slice(2);
  if (!/^[0-9a-f]*$/.test(hex)) {
    throw new Error(`${label} is not hex`);
  }
  if (hex.length % 2 === 1) hex = `0${hex}`;
  return `0x${hex}`;
}

function bytesFromHex(value: string, label: string): Uint8Array {
  const normalized = normalizeHex(value, label);
  return hexToBytes(normalized.slice(2));
}

function bytesToHex(bytes: Uint8Array): `0x${string}` {
  return `0x${Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("")}`;
}

function isNonceTooLowError(error: unknown): boolean {
  const message = error instanceof Error ? error.message : String(error);
  return message.toLowerCase().includes("nonce too low");
}
