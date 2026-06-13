import {
  DataKeyCommitmentUpdated,
  DataKeyShared,
  Joined,
  OracleRequestSkipped as OracleRequestSkippedEvent,
  VDDProofSubmitted
} from "../generated/templates/ExchangeChannelTemplate/ExchangeChannel";
import {
  Audience,
  DataKeyCommitment,
  DataKeyShare,
  OracleRequestSkipped,
  VddProof
} from "../generated/schema";
import { BigInt, Bytes } from "@graphprotocol/graph-ts";

function eventId(txHash: Bytes, logIndex: BigInt): Bytes {
  return txHash.concatI32(logIndex.toI32());
}

function audienceId(channel: Bytes, user: Bytes): Bytes {
  return channel.concat(user);
}

export function handleJoined(event: Joined): void {
  let entity = new Audience(audienceId(event.address, event.params.user));
  entity.channel = event.address;
  entity.user = event.params.user;
  entity.index = event.params.index;
  entity.joinedAtBlock = event.block.number;
  entity.joinedAtTimestamp = event.block.timestamp;
  entity.save();
}

export function handleDataKeyCommitmentUpdated(event: DataKeyCommitmentUpdated): void {
  let entity = new DataKeyCommitment(eventId(event.transaction.hash, event.logIndex));
  entity.channel = event.address;
  entity.commitment = event.params.newCommitment;
  entity.txHash = event.transaction.hash;
  entity.blockNumber = event.block.number;
  entity.timestamp = event.block.timestamp;
  entity.save();
}

export function handleDataKeyShared(event: DataKeyShared): void {
  let entity = new DataKeyShare(eventId(event.transaction.hash, event.logIndex));
  let audiences = new Array<Bytes>();
  for (let i = 0; i < event.params.audiences.length; i++) {
    audiences.push(event.params.audiences[i]);
  }

  entity.channel = event.address;
  entity.audiences = audiences;
  entity.encryptedDataKeys = event.params.encryptedDataKeys;
  entity.txHash = event.transaction.hash;
  entity.blockNumber = event.block.number;
  entity.timestamp = event.block.timestamp;
  entity.save();
}

export function handleVDDProofSubmitted(event: VDDProofSubmitted): void {
  let entity = new VddProof(eventId(event.transaction.hash, event.logIndex));
  entity.channel = event.address;
  entity.cCipher = event.params.cCipher;
  entity.txHash = event.transaction.hash;
  entity.blockNumber = event.block.number;
  entity.timestamp = event.block.timestamp;
  entity.save();
}

export function handleOracleRequestSkipped(event: OracleRequestSkippedEvent): void {
  let entity = new OracleRequestSkipped(eventId(event.transaction.hash, event.logIndex));
  entity.channel = event.address;
  entity.cCipher = event.params.cCipher;
  entity.message = event.params.msg;
  entity.txHash = event.transaction.hash;
  entity.blockNumber = event.block.number;
  entity.timestamp = event.block.timestamp;
  entity.save();
}
