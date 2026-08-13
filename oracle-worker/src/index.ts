import {
  createPublicClient,
  createWalletClient,
  decodeEventLog,
  encodeAbiParameters,
  getAddress,
  http,
  isAddress,
  parseAbi,
  type Hex,
  type Log,
} from "viem";
import { privateKeyToAccount } from "viem/accounts";
import { arbitrumSepolia } from "viem/chains";

interface Env {
  ARBITRUM_SEPOLIA_RPC_URL: string;
  ORACLE_PROXY_ADDRESS: string;
  ORACLE_RELAYER_PRIVATE_KEY: string;
  MIN_RELAYER_BALANCE_WEI: string;
  BLOCKBERRY_API_KEY: string;
  WORKER_API_TOKEN: string;
  MIN_CONFIRMATIONS?: string;
  CHAIN_ID?: string;
  BLOCKBERRY_WALRUS_BASE_URL?: string;
  WALRUS_AGGREGATOR_BASE_URL?: string;
}

type FulfillRequest = {
  chainId: number;
  txHash: Hex;
  requestLogIndex?: number;
  walrusEndEpoch?: number;
};

type WalrusAvailability = {
  blobId: string;
  found: boolean;
  expired: boolean;
  status: 0 | 1;
  statusName: "active" | "expired" | "not_found";
  endEpoch: number | null;
  endTime: number;
  expiresAt: string | null;
  upstreamStatus: number;
};

type ContractOracleStatus = 0 | 1 | 2;

const ORACLE_ABI = parseAbi([
  "event OracleRequested(bytes32 indexed requestId, address indexed client, bytes cid, uint256 nonce, uint8 mode)",
  "function requests(bytes32 requestId) view returns (bytes cid, address client, uint8 mode, bool fulfilled)",
  "function centralizedOracleSigner() view returns (address)",
  "function submitCentralizedReport(bytes report)",
]);

const INIT_DATE_SECONDS = Date.parse("2025-12-16T00:00:00Z") / 1000;
const INIT_EPOCH = 20;
const EPOCH_LENGTH_SECONDS = 1209600;
const ZERO_ADDRESS = "0x0000000000000000000000000000000000000000";

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    try {
      const url = new URL(request.url);
      if (url.pathname === "/health" && request.method === "GET") {
        return json({ ok: true });
      }
      if (url.pathname === "/status" && request.method === "GET") {
        requireAuth(request, env);
        return json(await status(env));
      }
      if (url.pathname === "/walrus/blob-status" && request.method === "GET") {
        requireAuth(request, env);
        return json(await walrusBlobStatus(url, env));
      }
      if (url.pathname === "/oracle/fulfill" && request.method === "POST") {
        requireAuth(request, env);
        return json(await fulfill(request, env));
      }
      return json({ ok: false, error: "NOT_FOUND" }, 404);
    } catch (error) {
      console.error("request failed", error);
      return json(errorBody(error), errorStatus(error));
    }
  },
};

async function status(env: Env) {
  const clients = makeClients(env);
  const [chainId, signer, balance, latestNonce, pendingNonce, onchainSigner] =
    await Promise.all([
      clients.publicClient.getChainId(),
      Promise.resolve(clients.account.address),
      clients.publicClient.getBalance({ address: clients.account.address }),
      clients.publicClient.getTransactionCount({
        address: clients.account.address,
        blockTag: "latest",
      }),
      clients.publicClient.getTransactionCount({
        address: clients.account.address,
        blockTag: "pending",
      }),
      clients.publicClient.readContract({
        address: clients.oracleProxy,
        abi: ORACLE_ABI,
        functionName: "centralizedOracleSigner",
      }),
    ]);

  const minBalance = parseBigIntEnv(env.MIN_RELAYER_BALANCE_WEI, "MIN_RELAYER_BALANCE_WEI");
  return {
    ok:
      chainId === expectedChainId(env) &&
      onchainSigner.toLowerCase() === signer.toLowerCase() &&
      balance >= minBalance &&
      latestNonce === pendingNonce,
    chainId,
    oracleProxyConfigured: clients.oracleProxy !== ZERO_ADDRESS,
    relayerConfigured: Boolean(env.ORACLE_RELAYER_PRIVATE_KEY),
    relayerMatchesOracleProxy: onchainSigner.toLowerCase() === signer.toLowerCase(),
    relayerBalanceSufficient: balance >= minBalance,
    relayerHasPendingTx: latestNonce !== pendingNonce,
    walrusApiConfigured: Boolean(env.BLOCKBERRY_API_KEY),
    lastCheckedAt: new Date().toISOString(),
  };
}

async function fulfill(request: Request, env: Env) {
  const body = await parseFulfillRequest(request);
  if (body.chainId !== expectedChainId(env)) {
    throw httpError("UNSUPPORTED_CHAIN", 400);
  }

  const clients = makeClients(env);
  await assertRelayerReady(env, clients);

  const receipt = await clients.publicClient.getTransactionReceipt({
    hash: body.txHash,
  });
  if (receipt.status !== "success") {
    throw httpError("TX_FAILED", 400);
  }
  const currentBlock = await clients.publicClient.getBlockNumber();
  const requiredConfirmations = BigInt(minConfirmations(env));
  const confirmations =
    currentBlock >= receipt.blockNumber ? currentBlock - receipt.blockNumber + 1n : 0n;
  if (confirmations < requiredConfirmations) {
    throw httpError("PENDING_CONFIRMATIONS", 425);
  }

  const requested = extractOracleRequest(receipt.logs, clients.oracleProxy, body.requestLogIndex);
  const onchain = await clients.publicClient.readContract({
    address: clients.oracleProxy,
    abi: ORACLE_ABI,
    functionName: "requests",
    args: [requested.requestId],
  });

  const [cid, client, mode, fulfilled] = onchain;
  if (fulfilled) {
    return {
      ok: true,
      alreadyFulfilled: true,
      requestId: requested.requestId,
    };
  }
  if (client.toLowerCase() !== requested.client.toLowerCase()) {
    throw httpError("REQUEST_CLIENT_MISMATCH", 400);
  }
  if (cid.toLowerCase() !== requested.cCipher.toLowerCase()) {
    throw httpError("REQUEST_CIPHER_MISMATCH", 400);
  }
  if (mode !== 0 || requested.mode !== 0) {
    throw httpError("REQUEST_NOT_CENTRALIZED", 400);
  }

  const availability = await checkWalrusAvailabilityByCipher(
    env,
    requested.cCipher,
    body.walrusEndEpoch,
  );
  const contractStatus = contractOracleStatus(availability);
  const report = encodeAbiParameters(
    [
      { name: "requestId", type: "bytes32" },
      { name: "cCipher", type: "bytes" },
      { name: "status", type: "uint256" },
      { name: "endTime", type: "uint256" },
      { name: "err", type: "bytes" },
    ],
    [
      requested.requestId,
      requested.cCipher,
      BigInt(contractStatus),
      BigInt(availability.endTime),
      "0x",
    ],
  );

  const hash = await clients.walletClient.writeContract({
    address: clients.oracleProxy,
    abi: ORACLE_ABI,
    functionName: "submitCentralizedReport",
    args: [report],
    account: clients.account,
    chain: arbitrumSepolia,
  });
  const reportReceipt = await clients.publicClient.waitForTransactionReceipt({
    hash,
    confirmations: minConfirmations(env),
  });
  if (reportReceipt.status !== "success") {
    throw httpError("REPORT_TX_REVERTED", 502);
  }

  return {
    ok: true,
    requestId: requested.requestId,
    reportTxHash: hash,
    status: contractStatus,
    walrusStatus: availability.status,
    walrusStatusName: availability.statusName,
    endTime: availability.endTime,
  };
}

async function walrusBlobStatus(url: URL, env: Env) {
  const blobId = parseWalrusBlobId(url);
  const availability = await checkWalrusBlobAvailability(env, blobId);
  return {
    ok: true,
    ...availability,
  };
}

function extractOracleRequest(logs: Log[], oracleProxy: Hex, requestLogIndex?: number) {
  const matches = logs
    .filter((log) => log.address.toLowerCase() === oracleProxy.toLowerCase())
    .map((log) => {
      try {
        const decoded = decodeEventLog({
          abi: ORACLE_ABI,
          data: log.data,
          topics: log.topics,
        });
        if (decoded.eventName !== "OracleRequested") return undefined;
        return {
          logIndex: log.logIndex,
          requestId: decoded.args.requestId,
          client: decoded.args.client,
          cCipher: decoded.args.cid,
          nonce: decoded.args.nonce,
          mode: decoded.args.mode,
        };
      } catch {
        return undefined;
      }
    })
    .filter((log): log is NonNullable<typeof log> => Boolean(log));

  if (requestLogIndex !== undefined) {
    const match = matches.find((log) => log.logIndex === requestLogIndex);
    if (!match) throw httpError("REQUEST_LOG_NOT_FOUND", 400);
    return match;
  }
  if (matches.length === 0) throw httpError("REQUEST_LOG_NOT_FOUND", 400);
  if (matches.length > 1) throw httpError("MULTIPLE_REQUEST_LOGS", 400);
  return matches[0];
}

async function checkWalrusAvailabilityByCipher(
  env: Env,
  cCipher: Hex,
  trustedEndEpoch?: number,
) {
  const blobId = hexToBase64Url(cCipher);
  if (trustedEndEpoch !== undefined) {
    const aggregatorAvailability = await checkWalrusAggregatorAvailability(env, blobId);
    if (aggregatorAvailability.status === 0) {
      return walrusAvailability(blobId, 0, trustedEndEpoch, aggregatorAvailability.upstreamStatus);
    }
  }
  return checkWalrusBlobAvailability(env, blobId);
}

async function checkWalrusAggregatorAvailability(
  env: Env,
  blobId: string,
): Promise<WalrusAvailability> {
  const baseUrl =
    env.WALRUS_AGGREGATOR_BASE_URL ?? "https://aggregator.walrus-mainnet.walrus.space";
  const response = await fetch(`${baseUrl.replace(/\/$/, "")}/v1/blobs/${blobId}`, {
    method: "HEAD",
  });
  if (!response.ok) {
    return walrusAvailability(blobId, 1, null, response.status);
  }
  return {
    blobId,
    found: true,
    expired: false,
    status: 0,
    statusName: "active",
    endEpoch: null,
    endTime: 0,
    expiresAt: null,
    upstreamStatus: response.status,
  };
}

async function checkWalrusBlobAvailability(
  env: Env,
  blobId: string,
): Promise<WalrusAvailability> {
  const baseUrl = env.BLOCKBERRY_WALRUS_BASE_URL ?? "https://api.blockberry.one/walrus-mainnet/v1";
  const response = await fetch(`${baseUrl}/blobs/${blobId}`, {
    headers: {
      accept: "*/*",
      "x-api-key": env.BLOCKBERRY_API_KEY,
    },
  });
  if (!response.ok) {
    return walrusAvailability(blobId, 1, null, response.status);
  }
  const data = await response.json<{ endEpoch?: number | string }>();
  const endEpoch = Number(data.endEpoch);
  if (!Number.isFinite(endEpoch)) {
    return walrusAvailability(blobId, 1, null, response.status);
  }
  const currentEpoch =
    INIT_EPOCH + (Date.now() / 1000 - INIT_DATE_SECONDS) / EPOCH_LENGTH_SECONDS;
  const status = endEpoch > currentEpoch ? 0 : 1;
  return walrusAvailability(blobId, status, endEpoch, response.status);
}

async function assertRelayerReady(env: Env, clients: ReturnType<typeof makeClients>) {
  const [latestNonce, pendingNonce, onchainSigner, balance] = await Promise.all([
    clients.publicClient.getTransactionCount({
      address: clients.account.address,
      blockTag: "latest",
    }),
    clients.publicClient.getTransactionCount({
      address: clients.account.address,
      blockTag: "pending",
    }),
    clients.publicClient.readContract({
      address: clients.oracleProxy,
      abi: ORACLE_ABI,
      functionName: "centralizedOracleSigner",
    }),
    clients.publicClient.getBalance({ address: clients.account.address }),
  ]);
  if (latestNonce !== pendingNonce) {
    throw httpError("RELAYER_PENDING_TX", 409);
  }
  if (onchainSigner.toLowerCase() !== clients.account.address.toLowerCase()) {
    throw httpError("RELAYER_NOT_AUTHORIZED", 500);
  }
  const minBalance = parseBigIntEnv(env.MIN_RELAYER_BALANCE_WEI, "MIN_RELAYER_BALANCE_WEI");
  if (balance < minBalance) {
    throw httpError("RELAYER_BALANCE_INSUFFICIENT", 503);
  }
}

function makeClients(env: Env) {
  const oracleProxy = normalizeAddress(env.ORACLE_PROXY_ADDRESS, "ORACLE_PROXY_ADDRESS");
  const account = privateKeyToAccount(normalizePrivateKey(env.ORACLE_RELAYER_PRIVATE_KEY));
  const transport = http(env.ARBITRUM_SEPOLIA_RPC_URL);
  return {
    oracleProxy,
    account,
    publicClient: createPublicClient({
      chain: arbitrumSepolia,
      transport,
    }),
    walletClient: createWalletClient({
      account,
      chain: arbitrumSepolia,
      transport,
    }),
  };
}

async function parseFulfillRequest(request: Request): Promise<FulfillRequest> {
  const body = await request.json<Partial<FulfillRequest>>();
  if (body.chainId !== 421614) throw httpError("UNSUPPORTED_CHAIN", 400);
  if (!isHex(body.txHash)) throw httpError("INVALID_TX_HASH", 400);
  if (
    body.requestLogIndex !== undefined &&
    (!Number.isInteger(body.requestLogIndex) || body.requestLogIndex < 0)
  ) {
    throw httpError("INVALID_REQUEST_LOG_INDEX", 400);
  }
  if (
    body.walrusEndEpoch !== undefined &&
    (!Number.isInteger(body.walrusEndEpoch) || body.walrusEndEpoch <= 0)
  ) {
    throw httpError("INVALID_WALRUS_END_EPOCH", 400);
  }
  return {
    chainId: body.chainId,
    txHash: body.txHash,
    requestLogIndex: body.requestLogIndex,
    walrusEndEpoch: body.walrusEndEpoch,
  };
}

function requireAuth(request: Request, env: Env) {
  if (!env.WORKER_API_TOKEN) throw httpError("WORKER_TOKEN_NOT_CONFIGURED", 500);
  const auth = request.headers.get("authorization") ?? "";
  const bearer = auth.startsWith("Bearer ") ? auth.slice("Bearer ".length) : "";
  const headerToken = request.headers.get("x-worker-token") ?? "";
  if (bearer !== env.WORKER_API_TOKEN && headerToken !== env.WORKER_API_TOKEN) {
    throw httpError("UNAUTHORIZED", 401);
  }
}

function hexToBase64Url(hex: Hex): string {
  const clean = hex.startsWith("0x") ? hex.slice(2) : hex;
  if (clean.length % 2 !== 0) throw httpError("INVALID_BLOB_ID_HEX", 400);
  let binary = "";
  for (let i = 0; i < clean.length; i += 2) {
    binary += String.fromCharCode(Number.parseInt(clean.slice(i, i + 2), 16));
  }
  return btoa(binary).replace(/\+/g, "-").replace(/\//g, "_").replace(/=/g, "");
}

function parseWalrusBlobId(url: URL): string {
  const blobId = url.searchParams.get("blobId");
  const cCipher = url.searchParams.get("cCipher");
  if (blobId && cCipher) throw httpError("AMBIGUOUS_BLOB_ID", 400);
  if (blobId) return normalizeWalrusBlobId(blobId);
  if (cCipher) {
    if (!isHex(cCipher)) throw httpError("INVALID_C_CIPHER", 400);
    return hexToBase64Url(cCipher);
  }
  throw httpError("MISSING_BLOB_ID", 400);
}

function normalizeWalrusBlobId(value: string): string {
  const blobId = value.trim();
  if (!/^[A-Za-z0-9_-]+$/.test(blobId)) {
    throw httpError("INVALID_WALRUS_BLOB_ID", 400);
  }
  return blobId;
}

function walrusAvailability(
  blobId: string,
  status: 0 | 1,
  endEpoch: number | null,
  upstreamStatus: number,
): WalrusAvailability {
  const endTime =
    endEpoch === null
      ? 0
      : Math.floor(INIT_DATE_SECONDS + (endEpoch - INIT_EPOCH) * EPOCH_LENGTH_SECONDS);
  const found = endEpoch !== null;
  const statusName = status === 0 ? "active" : found ? "expired" : "not_found";
  return {
    blobId,
    found,
    expired: status === 1 && found,
    status,
    statusName,
    endEpoch,
    endTime,
    expiresAt: endTime === 0 ? null : new Date(endTime * 1000).toISOString(),
    upstreamStatus,
  };
}

function contractOracleStatus(availability: WalrusAvailability): ContractOracleStatus {
  if (!availability.found || availability.expired) return 0;
  return availability.endTime > 0 ? 2 : 1;
}

function normalizePrivateKey(value: string): Hex {
  const key = value.startsWith("0x") ? value : `0x${value}`;
  if (!/^0x[0-9a-fA-F]{64}$/.test(key)) {
    throw httpError("INVALID_RELAYER_PRIVATE_KEY", 500);
  }
  return key as Hex;
}

function normalizeAddress(value: string, name: string): Hex {
  if (!isAddress(value)) throw httpError(`INVALID_${name}`, 500);
  return getAddress(value) as Hex;
}

function expectedChainId(env: Env): number {
  return Number(env.CHAIN_ID ?? "421614");
}

function minConfirmations(env: Env): number {
  const value = Number(env.MIN_CONFIRMATIONS ?? "1");
  return Number.isInteger(value) && value > 0 ? value : 1;
}

function parseBigIntEnv(value: string, name: string): bigint {
  try {
    return BigInt(value);
  } catch {
    throw httpError(`INVALID_${name}`, 500);
  }
}

function isHex(value: unknown): value is Hex {
  return typeof value === "string" && /^0x[0-9a-fA-F]+$/.test(value);
}

function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body, null, 2), {
    status,
    headers: {
      "content-type": "application/json; charset=utf-8",
      "cache-control": "no-store",
    },
  });
}

function httpError(code: string, status: number): Error & { code: string; status: number } {
  const error = new Error(code) as Error & { code: string; status: number };
  error.code = code;
  error.status = status;
  return error;
}

function errorStatus(error: unknown): number {
  return typeof error === "object" && error !== null && "status" in error
    ? Number((error as { status: unknown }).status)
    : 500;
}

function errorBody(error: unknown) {
  const code =
    typeof error === "object" && error !== null && "code" in error
      ? String((error as { code: unknown }).code)
      : "INTERNAL_ERROR";
  return { ok: false, error: code };
}
