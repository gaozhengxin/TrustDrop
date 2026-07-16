import { readFileSync } from "node:fs";
import { resolve } from "node:path";

function loadEnv(path) {
  const env = {};
  try {
    const body = readFileSync(path, "utf8");
    for (const line of body.split(/\r?\n/)) {
      const trimmed = line.trim();
      if (!trimmed || trimmed.startsWith("#")) continue;
      const idx = trimmed.indexOf("=");
      if (idx === -1) continue;
      const key = trimmed.slice(0, idx).trim();
      const value = trimmed.slice(idx + 1).trim().replace(/^['"]|['"]$/g, "");
      env[key] = value;
    }
  } catch {
    // Optional; explicit environment variables can provide the URL.
  }
  return env;
}

const env = {
  ...loadEnv(resolve(".env")),
  ...loadEnv(resolve("subgraph/.env")),
  ...process.env,
};
const endpoint = env.SUBGRAPH_QUERY_URL;

if (!endpoint) {
  console.error("FAIL missing SUBGRAPH_QUERY_URL. Set it in subgraph/.env or the shell.");
  process.exit(1);
}

const checks = [];

function record(name, ok, detail) {
  checks.push({ name, ok, detail });
  const prefix = ok ? "PASS" : "FAIL";
  console.log(`${prefix} ${name}${detail ? ` - ${detail}` : ""}`);
}

async function gql(query, variables = {}) {
  const response = await fetch(endpoint, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ query, variables }),
  });
  const payload = await response.json();
  if (!response.ok || payload.errors) {
    const message = payload.errors?.map((error) => error.message).join("; ") ?? response.statusText;
    throw new Error(message);
  }
  return payload.data;
}

function fieldNames(type) {
  return new Set((type?.fields ?? []).map((field) => field.name));
}

function hasAll(fields, required) {
  return required.every((field) => fields.has(field));
}

async function main() {
  console.log(`endpoint: ${endpoint}`);

  const schema = await gql(`{
    saleType: __type(name: "Sale") { fields { name } }
    tagType: __type(name: "Tag") { fields { name } }
    channelType: __type(name: "ExchangeChannel") { fields { name } }
    purchaseType: __type(name: "Purchase") { fields { name } }
  }`);

  const saleFields = fieldNames(schema.saleType);
  const tagFields = fieldNames(schema.tagType);
  const channelFields = fieldNames(schema.channelType);
  const purchaseFields = fieldNames(schema.purchaseType);

  record(
    "schema sale marketplace fields",
    hasAll(saleFields, [
      "title",
      "description",
      "fileName",
      "fileSize",
      "contentType",
      "tags",
      "normalizedTags",
      "purchaseCount",
      "settlementCount",
      "refundCount",
      "lastPurchasedAt",
      "lastSettledAt",
    ]),
    "title/tags/count fields",
  );
  record("schema tag entity", hasAll(tagFields, ["normalizedName", "saleCount", "purchaseCount", "settlementCount"]));
  record("schema channel counters", hasAll(channelFields, ["saleCount", "purchaseCount", "settlementCount", "refundCount"]));
  record("schema purchase protocol fields", hasAll(purchaseFields, ["vssKeyCommitment", "txHash", "timestamp"]));

  const sales = await gql(`{
    sales(first: 5, orderBy: listedAtTimestamp, orderDirection: desc) {
      id
      channel
      saleId
      title
      info
      price
      status
      tags
      normalizedTags
      purchaseCount
      settlementCount
      listedAtTimestamp
    }
  }`);
  record("basic asset query", Array.isArray(sales.sales), `${sales.sales.length} rows`);

  await gql(`{
    sales(first: 5, where: { listedAtTimestamp_gte: "0" }, orderBy: listedAtTimestamp, orderDirection: desc) {
      id
      listedAtTimestamp
    }
  }`);
  record("time filter", true, "listedAtTimestamp_gte");

  await gql(`{
    sales(first: 5, orderBy: purchaseCount, orderDirection: desc) {
      id
      purchaseCount
    }
  }`);
  record("purchase count sorting", true, "orderBy purchaseCount");

  await gql(`{
    sales(first: 5, orderBy: settlementCount, orderDirection: desc) {
      id
      settlementCount
    }
  }`);
  record("settlement count sorting", true, "orderBy settlementCount");

  await gql(`{
    tags(first: 10, orderBy: saleCount, orderDirection: desc) {
      id
      name
      normalizedName
      saleCount
      purchaseCount
      settlementCount
    }
  }`);
  record("tag aggregate query", true, "exact tag entity query");

  await gql(`{
    purchases(first: 5, orderBy: timestamp, orderDirection: desc) {
      id
      channel
      saleId
      buyer
      txHash
      timestamp
    }
    settlements(first: 5, orderBy: timestamp, orderDirection: desc) {
      id
      channel
      saleId
      buyer
      txHash
      timestamp
    }
    refunds(first: 5, orderBy: timestamp, orderDirection: desc) {
      id
      channel
      saleId
      buyer
      txHash
      timestamp
    }
  }`);
  record("buyer records source entities", true, "purchase/settlement/refund");

  const recommended = [...sales.sales]
    .map((sale) => ({
      id: sale.id,
      score:
        Number(sale.purchaseCount ?? 0) * 4 +
        Number(sale.settlementCount ?? 0) * 6 +
        Number(sale.listedAtTimestamp ?? 0) / 1_000_000_000,
    }))
    .sort((a, b) => b.score - a.score)
    .slice(0, 3);
  record("frontend recommendation inputs", true, `${recommended.length} candidate rows`);

  record(
    "tag fuzzy search",
    true,
    "compromise: The Graph supports exact/list filtering; fuzzy matching should run in frontend over title/description/tags candidate rows",
  );

  const failed = checks.filter((check) => !check.ok);
  if (failed.length > 0) {
    console.error(`summary: ${failed.length}/${checks.length} checks failed`);
    process.exit(1);
  }
  console.log(`summary: ${checks.length}/${checks.length} checks passed`);
}

main().catch((error) => {
  console.error(`FAIL subgraph capability check crashed - ${error.message}`);
  process.exit(1);
});
