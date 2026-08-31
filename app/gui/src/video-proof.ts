import { createPublicClient, encodeFunctionData, http, isAddress, parseAbi, type Hex } from "viem";
import { arbitrumSepolia } from "viem/chains";
import { DEFAULT_RPC_URL, type MarketplaceSale } from "../../../packages/drop-ts-sdk/src";

const VIDEO_PROOF_TAG_PREFIX = "trustdrop:video-sampling:v1:";
const CERTIFICATE_TYPE = "trustdrop.video-sampling";
const CERTIFICATE_VERSION = 1;
const IPFS_GATEWAYS = [
  "https://gateway.pinata.cloud/ipfs/",
  "https://dweb.link/ipfs/",
  "https://ipfs.io/ipfs/",
];
const FETCH_TIMEOUT_MS = 12_000;
const verifierAbi = parseAbi([
  "function verifyProof(bytes32 programVKey, bytes publicValues, bytes proofBytes) external view",
]);

export type VideoSamplingCertificate = {
  type: typeof CERTIFICATE_TYPE;
  version: typeof CERTIFICATE_VERSION;
  sale: {
    chainId: number;
    contract: Hex;
    saleId: Hex;
  };
  origin: {
    walrusBlobId: string;
  };
  sampling: {
    specHash: Hex;
    seed: Hex;
    externalRandomness: Hex;
    randomSource: string;
  };
  previews: Array<{
    bucket: number;
    cid: string;
  }>;
  proof: {
    system: "sp1-groth16";
    programVKey: Hex;
    publicValues: Hex;
    proofBytes: Hex;
  };
  verifier: {
    chainId: number;
    address: Hex;
    version: string;
  };
};

export type LoadedVideoProof = {
  certificateCid: string;
  certificateUrl: string;
  certificate: VideoSamplingCertificate;
  previewUrls: string[];
};

export function videoProofCid(tags: string[]): string | null {
  const tag = tags.find((value) => value.startsWith(VIDEO_PROOF_TAG_PREFIX));
  const cid = tag?.slice(VIDEO_PROOF_TAG_PREFIX.length) ?? "";
  return isCid(cid) ? cid : null;
}

export async function loadVideoProof(certificateCid: string, sale: MarketplaceSale): Promise<LoadedVideoProof> {
  if (!isCid(certificateCid)) throw new Error("Video proof tag contains an invalid certificate CID");
  const certificateResponse = await fetchIpfs(certificateCid, "application/json");
  const certificate = validateCertificate((await certificateResponse.response.json()) as unknown, sale);
  const previewUrls = await Promise.all(
    certificate.previews
      .slice()
      .sort((left, right) => left.bucket - right.bucket)
      .map(async (preview) => {
        const response = await fetchIpfs(preview.cid, "video/mp4", { Range: "bytes=0-31" });
        const bytes = new Uint8Array(await response.response.arrayBuffer());
        if (!isMp4Header(bytes)) throw new Error(`Preview ${preview.bucket + 1} is not an MP4 file`);
        return response.url;
      }),
  );
  return { certificateCid, certificateUrl: certificateResponse.url, certificate, previewUrls };
}

export async function verifyVideoProof(certificate: VideoSamplingCertificate): Promise<void> {
  const data = encodeFunctionData({
    abi: verifierAbi,
    functionName: "verifyProof",
    args: [certificate.proof.programVKey, certificate.proof.publicValues, certificate.proof.proofBytes],
  });
  const client = createPublicClient({ chain: arbitrumSepolia, transport: http(DEFAULT_RPC_URL) });
  const result = await client.call({ to: certificate.verifier.address, data });
  if (result.data !== "0x") throw new Error("Verifier returned unexpected data");
}

function validateCertificate(value: unknown, sale: MarketplaceSale): VideoSamplingCertificate {
  if (!isRecord(value)) throw new Error("Certificate is not a JSON object");
  if (value.type !== CERTIFICATE_TYPE || value.version !== CERTIFICATE_VERSION) {
    throw new Error("Unsupported video sampling certificate type or version");
  }
  const certificate = value as VideoSamplingCertificate;
  if (!isRecord(certificate.sale) || certificate.sale.chainId !== arbitrumSepolia.id) {
    throw new Error("Certificate uses the wrong chain");
  }
  if (!sameHex(certificate.sale.contract, sale.channel) || !sameHex(certificate.sale.saleId, sale.saleId)) {
    throw new Error("Certificate is not bound to this sale");
  }
  if (!isRecord(certificate.origin) || typeof certificate.origin.walrusBlobId !== "string" || !certificate.origin.walrusBlobId) {
    throw new Error("Certificate is missing the plaintext Walrus blob ID");
  }
  if (!isRecord(certificate.sampling) || !isHex32(certificate.sampling.specHash) || !isHex32(certificate.sampling.seed) || !isHex32(certificate.sampling.externalRandomness) || typeof certificate.sampling.randomSource !== "string" || !certificate.sampling.randomSource) {
    throw new Error("Certificate has invalid sampling inputs");
  }
  if (!Array.isArray(certificate.previews) || certificate.previews.length !== 3) {
    throw new Error("Certificate must contain exactly three previews");
  }
  const buckets = certificate.previews.map((preview) => preview.bucket).sort();
  if (buckets.join(",") !== "0,1,2" || certificate.previews.some((preview) => !isCid(preview.cid))) {
    throw new Error("Certificate has invalid preview entries");
  }
  if (!isRecord(certificate.proof) || certificate.proof.system !== "sp1-groth16" || !isHex32(certificate.proof.programVKey) || !isHex(certificate.proof.publicValues) || !isHex(certificate.proof.proofBytes)) {
    throw new Error("Certificate has invalid proof fixtures");
  }
  if (!isRecord(certificate.verifier) || certificate.verifier.chainId !== arbitrumSepolia.id || !isAddress(certificate.verifier.address) || typeof certificate.verifier.version !== "string") {
    throw new Error("Certificate has invalid verifier identity");
  }
  return certificate;
}

async function fetchIpfs(cid: string, accept: string, headers: Record<string, string> = {}): Promise<{ response: Response; url: string }> {
  const failures: string[] = [];
  for (const gateway of IPFS_GATEWAYS) {
    const url = `${gateway}${cid}`;
    const controller = new AbortController();
    const timeout = window.setTimeout(() => controller.abort(), FETCH_TIMEOUT_MS);
    try {
      const response = await fetch(url, { headers: { accept, ...headers }, signal: controller.signal });
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      return { response, url };
    } catch (error) {
      failures.push(`${gateway}: ${errorMessage(error)}`);
    } finally {
      window.clearTimeout(timeout);
    }
  }
  throw new Error(`IPFS fetch failed: ${failures.join("; ")}`);
}

function isMp4Header(bytes: Uint8Array): boolean {
  return bytes.length >= 12 && String.fromCharCode(...bytes.slice(4, 8)) === "ftyp";
}

function isCid(value: unknown): value is string {
  return typeof value === "string" && /^b[a-z2-7]{20,}$/.test(value);
}

function isHex(value: unknown): value is Hex {
  return typeof value === "string" && /^0x(?:[0-9a-fA-F]{2})+$/.test(value);
}

function isHex32(value: unknown): value is Hex {
  return typeof value === "string" && /^0x[0-9a-fA-F]{64}$/.test(value);
}

function sameHex(left: unknown, right: string): boolean {
  return typeof left === "string" && left.toLowerCase() === right.toLowerCase();
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
