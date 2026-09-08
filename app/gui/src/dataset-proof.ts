import { createPublicClient, decodeAbiParameters, encodeFunctionData, fromHex, http, isAddress, parseAbi, type Hex } from "viem";
import { arbitrumSepolia } from "viem/chains";
import { DEFAULT_RPC_URL, type MarketplaceSale } from "../../../packages/drop-ts-sdk/src";

type ImportMetaWithEnv = ImportMeta & { env?: Record<string, string | undefined> };

const TAG_PREFIX = "trustdrop:chain-intelligence-v2-sampling:v1:";
const CERTIFICATE_TYPE = "trustdrop.chain-intelligence-v2-sampling";
const DATASET_SCHEMA = "trustdrop.chain-intelligence.v2";
const FLOW_PROGRAM_VKEY = "0x00d47eb96b8c3846c8812056885424a768e4838a2796e98a2bf53fd7efef85ee";
const CLUSTER_PROGRAM_VKEY = "0x00749a17219133a8c64a776926fe18e7d6499072e27cd24ef182eecfd784434a";
const FLOW_VERIFIER_ADDRESS = "0xf8D06350C5b261e79ccA1A1061A6bd7922a3b09d";
const DEFAULT_CLUSTER_VERIFIER_ADDRESS = "0xe5Ba837f440AC5460C4cc02E5f48fe04994E68e6";
const CLUSTER_VERIFIER_ADDRESS = (import.meta as ImportMetaWithEnv).env?.VITE_CLUSTER_SAMPLING_VERIFIER_ADDRESS ?? DEFAULT_CLUSTER_VERIFIER_ADDRESS;
const flowVerifierAbi = parseAbi([
  "function verifyFlowGraphSamplingProof(bytes proof, bytes publicValues) external view returns ((bytes32 originBlobId, bytes32 samplingSeed, uint64 bucketStart, uint64 bucketEnd, bytes32 sampleCidDigest))",
]);
const clusterVerifierAbi = parseAbi([
  "function verifyClusterSamplingProof(bytes proof, bytes publicValues) external view returns ((bytes32 originBlobId, bytes32 samplingSeed, bytes32 clusterId0, bytes32 clusterId1, bytes32 clusterId2, bytes32 sampleCidDigest))",
]);

export type DatasetSampleProof = {
  cid: string;
  originBlobId: Hex;
  samplingSeed: Hex;
  sampleCidDigest: Hex;
  programVKey: Hex;
  publicValues: Hex;
  proof: Hex;
  verifier: { chainId: number; address: Hex; version: string };
};

export type DatasetSamplingCertificate = {
  type: typeof CERTIFICATE_TYPE;
  version: 1;
  sale: { chainId: number; contract: Hex; saleId: Hex };
  dataset: {
    schema: typeof DATASET_SCHEMA;
    chain: string;
    chainId: number;
    generatedAt: string;
    firstTimestamp: number;
    lastTimestamp: number;
    transferCount: number;
    clusteringMethod: "shared-sweep-destination-v1";
  };
  samples: {
    flow: DatasetSampleProof & { bucketStart: number; bucketEnd: number };
    clusters: DatasetSampleProof & { clusterIds: string[] };
  };
};

export type LoadedDatasetProof = {
  certificateCid: string;
  certificate: DatasetSamplingCertificate;
};

export function datasetProofCid(tags: string[]): string | null {
  const value = tags.find((tag) => tag.startsWith(TAG_PREFIX))?.slice(TAG_PREFIX.length) ?? "";
  return isCid(value) ? value : null;
}

export async function loadDatasetProof(cid: string, sale: MarketplaceSale): Promise<LoadedDatasetProof> {
  const response = await fetchIpfs(cid);
  const certificate = validateCertificate(await response.json(), sale);
  return { certificateCid: cid, certificate };
}

export function datasetSampleCalldata(kind: "flow" | "clusters", certificate: DatasetSamplingCertificate): Hex {
  const sample = certificate.samples[kind];
  if (kind === "flow") return encodeFunctionData({ abi: flowVerifierAbi, functionName: "verifyFlowGraphSamplingProof", args: [sample.proof, sample.publicValues] });
  return encodeFunctionData({
    abi: clusterVerifierAbi,
    functionName: "verifyClusterSamplingProof",
    args: [sample.proof, sample.publicValues],
  });
}

export async function verifyDatasetSample(kind: "flow" | "clusters", certificate: DatasetSamplingCertificate): Promise<void> {
  const sample = certificate.samples[kind];
  const client = createPublicClient({ chain: arbitrumSepolia, transport: http(DEFAULT_RPC_URL) });
  await client.call({ to: sample.verifier.address, data: datasetSampleCalldata(kind, certificate) });
}

export function datasetSampleCurlCommand(kind: "flow" | "clusters", certificate: DatasetSamplingCertificate): string {
  const sample = certificate.samples[kind];
  const body = JSON.stringify({ jsonrpc: "2.0", id: 1, method: "eth_call", params: [{ to: sample.verifier.address, data: datasetSampleCalldata(kind, certificate) }, "latest"] });
  return [
    `curl -sS '${DEFAULT_RPC_URL}' \\`,
    `  -H 'content-type: application/json' \\`,
    `  --data '${body}' \\`,
    `| python3 -c 'import json,sys; r=json.load(sys.stdin); print("Verified" if r.get("result", "").startswith("0x") else "Verification failed: " + str(r.get("error", r)))'`,
  ].join("\n");
}

export function datasetViewerUrl(kind: "flow" | "clusters", cid: string): string {
  const url = new URL("/chain-intelligence-viewer/", window.location.origin);
  url.search = new URLSearchParams({ view: kind === "flow" ? "flow" : "cluster", cid }).toString();
  return url.toString();
}

function validateCertificate(value: unknown, sale: MarketplaceSale): DatasetSamplingCertificate {
  if (!isRecord(value) || value.type !== CERTIFICATE_TYPE || value.version !== 1) throw new Error("Unsupported chain-intelligence sampling certificate");
  const certificate = value as DatasetSamplingCertificate;
  if (!isRecord(certificate.sale) || certificate.sale.chainId !== arbitrumSepolia.id || !sameHex(certificate.sale.contract, sale.channel) || !sameHex(certificate.sale.saleId, sale.saleId)) throw new Error("Certificate is not bound to this sale");
  if (!isRecord(certificate.dataset) || certificate.dataset.schema !== DATASET_SCHEMA || certificate.dataset.chainId < 1 || certificate.dataset.clusteringMethod !== "shared-sweep-destination-v1") throw new Error("Certificate does not describe the supported chain-intelligence v2 dataset");
  validateSample(certificate.samples?.flow, "flow");
  validateSample(certificate.samples?.clusters, "clusters");
  if (!sameHex(certificate.samples.flow.programVKey, FLOW_PROGRAM_VKEY) || !sameHex(certificate.samples.flow.verifier.address, FLOW_VERIFIER_ADDRESS)) throw new Error("Flow proof does not use the trusted program and verifier");
  if (!isAddress(CLUSTER_VERIFIER_ADDRESS) || !sameHex(certificate.samples.clusters.programVKey, CLUSTER_PROGRAM_VKEY) || !sameHex(certificate.samples.clusters.verifier.address, CLUSTER_VERIFIER_ADDRESS)) throw new Error("Cluster proof does not use the trusted program and verifier");
  if (!Number.isSafeInteger(certificate.samples.flow.bucketStart) || !Number.isSafeInteger(certificate.samples.flow.bucketEnd) || certificate.samples.flow.bucketEnd <= certificate.samples.flow.bucketStart) throw new Error("Flow sample has an invalid time range");
  if (!Array.isArray(certificate.samples.clusters.clusterIds) || certificate.samples.clusters.clusterIds.length !== 3 || certificate.samples.clusters.clusterIds.some((id) => !/^wcl_[0-9a-f]{24}$/.test(id))) throw new Error("Cluster sample must contain three canonical cluster IDs");
  validatePublicValues(certificate);
  return certificate;
}

function validatePublicValues(certificate: DatasetSamplingCertificate): void {
  const flow = decodeAbiParameters(
    [{ type: "bytes32" }, { type: "bytes32" }, { type: "uint64" }, { type: "uint64" }, { type: "bytes32" }],
    certificate.samples.flow.publicValues,
  );
  if (!sameHex(flow[0], certificate.samples.flow.originBlobId) || !sameHex(flow[1], certificate.samples.flow.samplingSeed) || flow[2] !== BigInt(certificate.samples.flow.bucketStart) || flow[3] !== BigInt(certificate.samples.flow.bucketEnd) || !sameHex(flow[4], certificate.samples.flow.sampleCidDigest)) throw new Error("Flow certificate fields do not match the proven public values");

  const clusters = decodeAbiParameters(
    [{ type: "bytes32" }, { type: "bytes32" }, { type: "bytes32" }, { type: "bytes32" }, { type: "bytes32" }, { type: "bytes32" }],
    certificate.samples.clusters.publicValues,
  );
  const provenClusterIds = clusters.slice(2, 5).map((value) => fromHex(value, "string").replace(/\0+$/, ""));
  if (!sameHex(clusters[0], certificate.samples.clusters.originBlobId) || !sameHex(clusters[1], certificate.samples.clusters.samplingSeed) || provenClusterIds.some((id, index) => id !== certificate.samples.clusters.clusterIds[index]) || !sameHex(clusters[5], certificate.samples.clusters.sampleCidDigest)) throw new Error("Cluster certificate fields do not match the proven public values");
}

function validateSample(value: unknown, label: string): asserts value is DatasetSampleProof {
  if (!isRecord(value) || !isCid(value.cid) || !isHex32(value.originBlobId) || !isHex32(value.samplingSeed) || !isHex32(value.sampleCidDigest) || !isHex32(value.programVKey) || !isHex(value.publicValues) || !isHex(value.proof) || !isRecord(value.verifier) || value.verifier.chainId !== arbitrumSepolia.id || !isAddress(value.verifier.address) || typeof value.verifier.version !== "string") throw new Error(`${label} sample proof is malformed`);
  if (!sameHex(value.sampleCidDigest, `0x${value.publicValues.slice(-64)}`)) throw new Error(`${label} sample digest is not committed by its public values`);
  if (!sameHex(value.sampleCidDigest, cidDigest(value.cid))) throw new Error(`${label} sample CID does not match its proof digest`);
}

async function fetchIpfs(cid: string): Promise<Response> {
  const urls = [`https://${cid}.ipfs.dweb.link/`, `https://gateway.pinata.cloud/ipfs/${cid}`, `https://ipfs.io/ipfs/${cid}`];
  for (const url of urls) {
    try {
      const response = await fetch(url, { headers: { accept: "application/json" } });
      if (response.ok) return response;
    } catch {}
  }
  throw new Error(`Unable to fetch certificate CID ${cid}`);
}

function cidDigest(cid: string): Hex {
  const alphabet = "abcdefghijklmnopqrstuvwxyz234567";
  let bits = 0;
  let value = 0;
  const bytes: number[] = [];
  for (const char of cid.slice(1)) {
    const digit = alphabet.indexOf(char);
    if (digit < 0) throw new Error("CID is not canonical base32");
    value = (value << 5) | digit;
    bits += 5;
    if (bits >= 8) { bits -= 8; bytes.push((value >>> bits) & 0xff); }
  }
  if (bytes.length < 36 || bytes[bytes.length - 34] !== 0x12 || bytes[bytes.length - 33] !== 0x20) throw new Error("CID does not use sha2-256");
  return `0x${bytes.slice(-32).map((byte) => byte.toString(16).padStart(2, "0")).join("")}`;
}

function isCid(value: unknown): value is string { return typeof value === "string" && /^b[a-z2-7]{20,}$/.test(value); }
function isHex(value: unknown): value is Hex { return typeof value === "string" && /^0x(?:[0-9a-fA-F]{2})+$/.test(value); }
function isHex32(value: unknown): value is Hex { return typeof value === "string" && /^0x[0-9a-fA-F]{64}$/.test(value); }
function sameHex(left: unknown, right: string): boolean { return typeof left === "string" && left.toLowerCase() === right.toLowerCase(); }
function isRecord(value: unknown): value is Record<string, any> { return typeof value === "object" && value !== null && !Array.isArray(value); }
