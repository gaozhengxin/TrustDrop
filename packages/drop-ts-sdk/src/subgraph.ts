import { DEFAULT_SUBGRAPH_URL } from "./config";

export type MarketplaceSale = {
  id: string;
  channel: `0x${string}`;
  saleId: `0x${string}`;
  dataCommitment: `0x${string}`;
  price: string;
  version: `0x${string}`;
  info: string;
  title: string;
  description: string;
  fileName: string;
  fileSize: string;
  contentType: string;
  tags: string[];
  normalizedTags: string[];
  purchaseCount: string;
  settlementCount: string;
  refundCount: string;
  listedAtBlock: string;
  listedAtTimestamp: string;
  updatedAtBlock: string;
  updatedAtTimestamp: string;
  status: string;
};

export type MarketplacePurchase = {
  id: string;
  channel: `0x${string}`;
  saleId: `0x${string}`;
  dataCommitment: `0x${string}`;
  buyer: `0x${string}`;
  price: string;
  saleDigest: `0x${string}`;
  initTime: string;
  deadline: string;
  vssKeyCommitment: `0x${string}`;
  txHash: `0x${string}`;
  blockNumber: string;
  timestamp: string;
};

export type MarketplaceSettlement = {
  id: string;
  channel: `0x${string}`;
  buyer: `0x${string}`;
  saleId: `0x${string}`;
  dataCommitment: `0x${string}`;
  txHash: `0x${string}`;
  blockNumber: string;
  timestamp: string;
};

export type MarketplaceRefund = {
  id: string;
  channel: `0x${string}`;
  buyer: `0x${string}`;
  saleId: `0x${string}`;
  dataCommitment: `0x${string}`;
  amount: string;
  txHash: `0x${string}`;
  blockNumber: string;
  timestamp: string;
};

export type DataKeyShare = {
  id: string;
  channel: `0x${string}`;
  audiences: `0x${string}`[];
  encryptedDataKeys: `0x${string}`[];
  txHash: `0x${string}`;
  blockNumber: string;
  timestamp: string;
};

export type VddProof = {
  id: string;
  channel: `0x${string}`;
  cCipher: `0x${string}`;
  txHash: `0x${string}`;
  blockNumber: string;
  timestamp: string;
};

type GraphqlResponse<T> = {
  data?: T;
  errors?: Array<{ message: string }>;
};

export class TrustDropSubgraph {
  readonly endpoint: string;

  constructor(endpoint = DEFAULT_SUBGRAPH_URL) {
    this.endpoint = endpoint;
  }

  async query<T>(query: string, variables: Record<string, unknown> = {}): Promise<T> {
    const response = await fetch(this.endpoint, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ query, variables }),
    });
    const payload = (await response.json()) as GraphqlResponse<T>;
    if (!response.ok || payload.errors) {
      const message = payload.errors?.map((error) => error.message).join("; ") ?? response.statusText;
      throw new Error(message);
    }
    if (!payload.data) throw new Error("Subgraph returned no data");
    return payload.data;
  }

  async listSales(first = 24): Promise<MarketplaceSale[]> {
    const result = await this.query<{ sales: MarketplaceSale[] }>(
      `query Sales($first: Int!) {
        sales(first: $first, orderBy: listedAtTimestamp, orderDirection: desc, where: { status: "LISTED" }) {
          id channel saleId dataCommitment price version info title description fileName fileSize contentType
          tags normalizedTags purchaseCount settlementCount refundCount listedAtBlock listedAtTimestamp updatedAtBlock updatedAtTimestamp status
        }
      }`,
      { first },
    );
    return result.sales;
  }

  async getSale(channel: `0x${string}`, saleId: `0x${string}`): Promise<MarketplaceSale | null> {
    const result = await this.query<{ sales: MarketplaceSale[] }>(
      `query Sale($channel: Bytes!, $saleId: Bytes!) {
        sales(first: 1, where: { channel: $channel, saleId: $saleId }) {
          id channel saleId dataCommitment price version info title description fileName fileSize contentType
          tags normalizedTags purchaseCount settlementCount refundCount listedAtBlock listedAtTimestamp updatedAtBlock updatedAtTimestamp status
        }
      }`,
      { channel: channel.toLowerCase(), saleId: saleId.toLowerCase() },
    );
    return result.sales[0] ?? null;
  }

  async listBuyerActivity(buyer: `0x${string}`): Promise<{
    purchases: MarketplacePurchase[];
    settlements: MarketplaceSettlement[];
    refunds: MarketplaceRefund[];
    dataKeyShares: DataKeyShare[];
    vddProofs: VddProof[];
  }> {
    const result = await this.query<{
      purchases: MarketplacePurchase[];
      settlements: MarketplaceSettlement[];
      refunds: MarketplaceRefund[];
      dataKeyShares: DataKeyShare[];
      vddProofs: VddProof[];
    }>(
      `query BuyerActivity($buyer: Bytes!) {
        purchases(first: 50, orderBy: timestamp, orderDirection: desc, where: { buyer: $buyer }) {
          id channel saleId dataCommitment buyer price saleDigest initTime deadline vssKeyCommitment txHash blockNumber timestamp
        }
        settlements(first: 50, orderBy: timestamp, orderDirection: desc, where: { buyer: $buyer }) {
          id channel buyer saleId dataCommitment txHash blockNumber timestamp
        }
        refunds(first: 50, orderBy: timestamp, orderDirection: desc, where: { buyer: $buyer }) {
          id channel buyer saleId dataCommitment amount txHash blockNumber timestamp
        }
        dataKeyShares(first: 100, orderBy: timestamp, orderDirection: desc) {
          id channel audiences encryptedDataKeys txHash blockNumber timestamp
        }
        vddProofs(first: 100, orderBy: timestamp, orderDirection: desc) {
          id channel cCipher txHash blockNumber timestamp
        }
      }`,
      { buyer: buyer.toLowerCase() },
    );
    const channels = new Set(result.purchases.map((purchase) => purchase.channel.toLowerCase()));
    return {
      ...result,
      dataKeyShares: result.dataKeyShares.filter((share) =>
        share.audiences.some((audience) => audience.toLowerCase() === buyer.toLowerCase()),
      ),
      vddProofs: result.vddProofs.filter((proof) => channels.has(proof.channel.toLowerCase())),
    };
  }
}
