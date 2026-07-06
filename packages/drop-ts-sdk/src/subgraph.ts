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
  listedAtTimestamp: string;
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
  ephemeralPubkey: `0x${string}`;
  txHash: `0x${string}`;
  timestamp: string;
};

export type MarketplaceSettlement = {
  id: string;
  channel: `0x${string}`;
  buyer: `0x${string}`;
  saleId: `0x${string}`;
  dataCommitment: `0x${string}`;
  txHash: `0x${string}`;
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
          tags normalizedTags purchaseCount settlementCount refundCount listedAtTimestamp updatedAtTimestamp status
        }
      }`,
      { first },
    );
    return result.sales;
  }

  async listBuyerActivity(buyer: `0x${string}`): Promise<{
    purchases: MarketplacePurchase[];
    settlements: MarketplaceSettlement[];
    refunds: MarketplaceRefund[];
  }> {
    const result = await this.query<{
      purchases: MarketplacePurchase[];
      settlements: MarketplaceSettlement[];
      refunds: MarketplaceRefund[];
    }>(
      `query BuyerActivity($buyer: Bytes!) {
        purchases(first: 50, orderBy: timestamp, orderDirection: desc, where: { buyer: $buyer }) {
          id channel saleId dataCommitment buyer price saleDigest initTime deadline vssKeyCommitment ephemeralPubkey txHash timestamp
        }
        settlements(first: 50, orderBy: timestamp, orderDirection: desc, where: { buyer: $buyer }) {
          id channel buyer saleId dataCommitment txHash timestamp
        }
        refunds(first: 50, orderBy: timestamp, orderDirection: desc, where: { buyer: $buyer }) {
          id channel buyer saleId dataCommitment amount txHash timestamp
        }
      }`,
      { buyer: buyer.toLowerCase() },
    );
    return result;
  }
}
