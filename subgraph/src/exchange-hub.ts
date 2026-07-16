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
  Settlement,
  Tag
} from "../generated/schema";
import { BigInt, Bytes, JSONValue, JSONValueKind, json } from "@graphprotocol/graph-ts";

function eventId(txHash: Bytes, logIndex: BigInt): Bytes {
  return txHash.concatI32(logIndex.toI32());
}

function saleEntityId(channel: Bytes, saleId: Bytes): Bytes {
  return channel.concat(saleId);
}

function zero(): BigInt {
  return BigInt.fromI32(0);
}

function one(): BigInt {
  return BigInt.fromI32(1);
}

function getStringField(root: JSONValue, key: string): string {
  if (root.kind != JSONValueKind.OBJECT) return "";
  let value = root.toObject().get(key);
  if (value == null || value.kind != JSONValueKind.STRING) return "";
  return value.toString();
}

function getU64Field(root: JSONValue, key: string): BigInt {
  if (root.kind != JSONValueKind.OBJECT) return zero();
  let value = root.toObject().get(key);
  if (value == null || value.kind != JSONValueKind.NUMBER) return zero();
  return BigInt.fromU64(value.toU64());
}

function normalizeTag(tag: string): string {
  return tag.trim().toLowerCase();
}

function getTagsField(root: JSONValue): Array<string> {
  let tags = new Array<string>();
  if (root.kind != JSONValueKind.OBJECT) return tags;
  let value = root.toObject().get("tags");
  if (value == null || value.kind != JSONValueKind.ARRAY) return tags;

  let values = value.toArray();
  for (let i = 0; i < values.length; i++) {
    if (values[i].kind != JSONValueKind.STRING) continue;
    let tag = values[i].toString().trim();
    if (tag.length > 0) tags.push(tag);
  }
  return tags;
}

function applySaleMetadata(entity: Sale, info: string): void {
  entity.info = info;
  entity.title = info.length > 0 ? info : "Untitled file";
  entity.description = "";
  entity.fileName = "";
  entity.fileSize = zero();
  entity.contentType = "";
  entity.tags = new Array<string>();
  entity.normalizedTags = new Array<string>();

  let parsed = json.try_fromString(info);
  if (parsed.isError) return;

  let root = parsed.value;
  let title = getStringField(root, "title");
  let description = getStringField(root, "description");
  let fileName = getStringField(root, "fileName");
  let contentType = getStringField(root, "contentType");
  let tags = getTagsField(root);

  if (title.length > 0) entity.title = title;
  if (description.length > 0) entity.description = description;
  if (fileName.length > 0) entity.fileName = fileName;
  entity.fileSize = getU64Field(root, "fileSize");
  if (contentType.length > 0) entity.contentType = contentType;

  let normalizedTags = new Array<string>();
  for (let i = 0; i < tags.length; i++) {
    let normalized = normalizeTag(tags[i]);
    if (normalized.length > 0) normalizedTags.push(normalized);
  }
  entity.tags = tags;
  entity.normalizedTags = normalizedTags;
}

function initializeSaleCounters(entity: Sale): void {
  entity.purchaseCount = zero();
  entity.settlementCount = zero();
  entity.refundCount = zero();
  entity.lastPurchasedAt = null;
  entity.lastSettledAt = null;
  entity.lastRefundedAt = null;
}

function upsertTagForSale(tagName: string): void {
  let normalized = normalizeTag(tagName);
  if (normalized.length == 0) return;

  let entity = Tag.load(normalized);
  if (entity == null) {
    entity = new Tag(normalized);
    entity.name = tagName;
    entity.normalizedName = normalized;
    entity.saleCount = zero();
    entity.purchaseCount = zero();
    entity.settlementCount = zero();
  }
  entity.saleCount = entity.saleCount.plus(one());
  entity.save();
}

function incrementTagPurchaseCounters(tags: Array<string>): void {
  for (let i = 0; i < tags.length; i++) {
    let normalized = normalizeTag(tags[i]);
    let entity = Tag.load(normalized);
    if (entity == null) continue;
    entity.purchaseCount = entity.purchaseCount.plus(one());
    entity.save();
  }
}

function incrementTagSettlementCounters(tags: Array<string>): void {
  for (let i = 0; i < tags.length; i++) {
    let normalized = normalizeTag(tags[i]);
    let entity = Tag.load(normalized);
    if (entity == null) continue;
    entity.settlementCount = entity.settlementCount.plus(one());
    entity.save();
  }
}

export function handleExchangeChannelCreated(event: ExchangeChannelCreated): void {
  let entity = new ExchangeChannel(event.params.channel);
  entity.owner = event.params.owner;
  entity.channel = event.params.channel;
  entity.saleCount = zero();
  entity.purchaseCount = zero();
  entity.settlementCount = zero();
  entity.refundCount = zero();
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
  applySaleMetadata(entity, event.params.info);
  initializeSaleCounters(entity);
  entity.status = "LISTED";
  entity.listedAtBlock = event.block.number;
  entity.listedAtTimestamp = event.block.timestamp;
  entity.updatedAtBlock = event.block.number;
  entity.updatedAtTimestamp = event.block.timestamp;
  entity.save();

  let channel = ExchangeChannel.load(event.params.channel);
  if (channel != null) {
    channel.saleCount = channel.saleCount.plus(one());
    channel.save();
  }

  for (let i = 0; i < entity.tags.length; i++) {
    upsertTagForSale(entity.tags[i]);
  }
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
    initializeSaleCounters(entity);
  }
  entity.dataCommitment = event.params.dataCommitment;
  entity.price = event.params.newPrice;
  entity.version = event.params.version;
  applySaleMetadata(entity, event.params.info);
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

  let sale = Sale.load(saleEntityId(event.params.channel, event.params.saleId));
  if (sale != null) {
    sale.purchaseCount = sale.purchaseCount.plus(one());
    sale.lastPurchasedAt = event.block.timestamp;
    sale.updatedAtBlock = event.block.number;
    sale.updatedAtTimestamp = event.block.timestamp;
    sale.save();
    incrementTagPurchaseCounters(sale.tags);
  }

  let channel = ExchangeChannel.load(event.params.channel);
  if (channel != null) {
    channel.purchaseCount = channel.purchaseCount.plus(one());
    channel.save();
  }
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
    sale.settlementCount = sale.settlementCount.plus(one());
    sale.lastSettledAt = event.block.timestamp;
    sale.updatedAtBlock = event.block.number;
    sale.updatedAtTimestamp = event.block.timestamp;
    sale.save();
    incrementTagSettlementCounters(sale.tags);
  }

  let channel = ExchangeChannel.load(event.params.channel);
  if (channel != null) {
    channel.settlementCount = channel.settlementCount.plus(one());
    channel.save();
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
    sale.refundCount = sale.refundCount.plus(one());
    sale.lastRefundedAt = event.block.timestamp;
    sale.updatedAtBlock = event.block.number;
    sale.updatedAtTimestamp = event.block.timestamp;
    sale.save();
  }

  let channel = ExchangeChannel.load(event.params.channel);
  if (channel != null) {
    channel.refundCount = channel.refundCount.plus(one());
    channel.save();
  }
}
