import type { BuyerThread, MarketplacePurchase, MarketplaceRefund, MarketplaceSale, MarketplaceSettlement } from "../../../../packages/drop-ts-sdk/src";
import rule0 from "./rules/0.json";

type ContentRuleDescriptor = {
  id: string;
  name: string;
  version: number;
  updatedAt: string;
  filters: {
    minimumListedBlock: string;
    boundarySaleId: string;
    boundaryListTx: string;
  };
  recommendation: {
    mode: string;
  };
  moderation: {
    mode: string;
  };
};

type BuyerActivity = {
  purchases: MarketplacePurchase[];
  settlements: MarketplaceSettlement[];
  refunds: MarketplaceRefund[];
};

const descriptor = rule0 as ContentRuleDescriptor;

export function activeContentRule(): ContentRuleDescriptor {
  return descriptor;
}

export function filterSalesForContentEngine(sales: MarketplaceSale[]): MarketplaceSale[] {
  const minimumListedBlock = BigInt(descriptor.filters.minimumListedBlock);
  return sales.filter((sale) => parseBigInt(sale.listedAtBlock) >= minimumListedBlock);
}

export function filterBuyerActivityForContentEngine(activity: BuyerActivity, visibleSales: MarketplaceSale[]): BuyerActivity {
  const visibleSaleKeys = new Set(visibleSales.map(saleKey));
  return {
    purchases: activity.purchases.filter((purchase) => visibleSaleKeys.has(eventKey(purchase))),
    settlements: activity.settlements.filter((settlement) => visibleSaleKeys.has(eventKey(settlement))),
    refunds: activity.refunds.filter((refund) => visibleSaleKeys.has(eventKey(refund))),
  };
}

export function filterBuyerThreadsForContentEngine(threads: BuyerThread[], visibleSales: MarketplaceSale[]): BuyerThread[] {
  const visibleSaleKeys = new Set(visibleSales.map(saleKey));
  return threads.filter((thread) => visibleSaleKeys.has(eventKey(thread)));
}

function saleKey(sale: MarketplaceSale): string {
  return `${sale.channel}:${sale.saleId}`.toLowerCase();
}

function eventKey(event: MarketplacePurchase | MarketplaceSettlement | MarketplaceRefund | BuyerThread): string {
  return `${event.channel}:${event.saleId}`.toLowerCase();
}

function parseBigInt(value: string): bigint {
  try {
    return BigInt(value);
  } catch {
    return 0n;
  }
}
