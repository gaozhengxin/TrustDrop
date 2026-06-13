import {
  ExchangeChannelCreated,
  RefundEvent,
  SaleDelisted,
  SaleListed,
  SaleUpdated,
  PurchaseEvent,
  SettleEvent
} from "../generated/ExchangeHub/ExchangeHub";
import { ExchangeChannelTemplate } from "../generated/templates";
import {
  ExchangeChannel,
  Purchase,
  Refund,
  Sale,
  Settlement
} from "../generated/schema";
import { BigInt, Bytes } from "@graphprotocol/graph-ts";

function eventId(txHash: Bytes, logIndex: BigInt): Bytes {
  return txHash.concatI32(logIndex.toI32());
}

function saleEntityId(channel: Bytes, saleId: Bytes): Bytes {
  return channel.concat(saleId);
}

export function handleExchangeChannelCreated(event: ExchangeChannelCreated): void {
  let entity = new ExchangeChannel(event.params.channel);
  entity.owner = event.params.owner;
  entity.channel = event.params.channel;
  entity.createdAtBlock = event.block.number;
  entity.createdAtTimestamp = event.block.timestamp;
  entity.createdTx = event.transaction.hash;
  entity.save();

  ExchangeChannelTemplate.create(event.params.channel);
}

export function handleSaleListed(event: SaleListed): void {
  let id = saleEntityId(event.params.channel, event.params.saleId);
  let entity = new Sale(id);
  entity.channel = event.params.channel;
  entity.saleId = event.params.saleId;
  entity.dataCommitment = event.params.dataCommitment;
  entity.price = event.params.price;
  entity.version = event.params.version;
  entity.info = event.params.info;
  entity.status = "LISTED";
  entity.listedAtBlock = event.block.number;
  entity.listedAtTimestamp = event.block.timestamp;
  entity.updatedAtBlock = event.block.number;
  entity.updatedAtTimestamp = event.block.timestamp;
  entity.save();
}

export function handleSaleUpdated(event: SaleUpdated): void {
  let id = saleEntityId(event.params.channel, event.params.saleId);
  let entity = Sale.load(id);
  if (entity == null) {
    entity = new Sale(id);
    entity.channel = event.params.channel;
    entity.saleId = event.params.saleId;
    entity.listedAtBlock = event.block.number;
    entity.listedAtTimestamp = event.block.timestamp;
  }
  entity.dataCommitment = event.params.dataCommitment;
  entity.price = event.params.newPrice;
  entity.version = event.params.version;
  entity.info = event.params.info;
  entity.status = "LISTED";
  entity.updatedAtBlock = event.block.number;
  entity.updatedAtTimestamp = event.block.timestamp;
  entity.save();
}

export function handleSaleDelisted(event: SaleDelisted): void {
  let id = saleEntityId(event.params.channel, event.params.saleId);
  let entity = Sale.load(id);
  if (entity == null) return;
  entity.status = "DELISTED";
  entity.updatedAtBlock = event.block.number;
  entity.updatedAtTimestamp = event.block.timestamp;
  entity.save();
}

export function handlePurchaseEvent(event: PurchaseEvent): void {
  let entity = new Purchase(eventId(event.transaction.hash, event.logIndex));
  entity.channel = event.params.channel;
  entity.saleId = event.params.saleId;
  entity.dataCommitment = event.params.dataCommitment;
  entity.buyer = event.params.buyer;
  entity.price = event.params.price;
  entity.saleDigest = event.params.exchangeInfo.saleDigest;
  entity.initTime = event.params.exchangeInfo.initTime;
  entity.deadline = event.params.exchangeInfo.deadline;
  entity.vssKeyCommitment = event.params.exchangeInfo.vssKeyCommitment;
  entity.txHash = event.transaction.hash;
  entity.blockNumber = event.block.number;
  entity.timestamp = event.block.timestamp;
  entity.save();
}

export function handleSettleEvent(event: SettleEvent): void {
  let entity = new Settlement(eventId(event.transaction.hash, event.logIndex));
  entity.channel = event.params.channel;
  entity.buyer = event.params.buyer;
  entity.saleId = event.params.saleId;
  entity.dataCommitment = event.params.dataCommitment;
  entity.txHash = event.transaction.hash;
  entity.blockNumber = event.block.number;
  entity.timestamp = event.block.timestamp;
  entity.save();

  let sale = Sale.load(saleEntityId(event.params.channel, event.params.saleId));
  if (sale != null) {
    sale.status = "SETTLED";
    sale.updatedAtBlock = event.block.number;
    sale.updatedAtTimestamp = event.block.timestamp;
    sale.save();
  }
}

export function handleRefundEvent(event: RefundEvent): void {
  let entity = new Refund(eventId(event.transaction.hash, event.logIndex));
  entity.channel = event.params.channel;
  entity.buyer = event.params.buyer;
  entity.saleId = event.params.saleId;
  entity.dataCommitment = event.params.dataCommitment;
  entity.amount = event.params.amount;
  entity.txHash = event.transaction.hash;
  entity.blockNumber = event.block.number;
  entity.timestamp = event.block.timestamp;
  entity.save();

  let sale = Sale.load(saleEntityId(event.params.channel, event.params.saleId));
  if (sale != null) {
    sale.status = "REFUNDED";
    sale.updatedAtBlock = event.block.number;
    sale.updatedAtTimestamp = event.block.timestamp;
    sale.save();
  }
}
