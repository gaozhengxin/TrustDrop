const cid = args[0];

// 1. 验证 IPFS 可检索性
const ipfsRequest = Functions.makeHttpRequest({
  url: `https://ipfs.io/ipfs/${cid}?format=dag-json`,
  headers: { "Accept": "application/vnd.ipld.dag-json" },
  timeout: 4000
});

// 2. 获取 Lighthouse Deal 状态
const lighthouseRequest = Functions.makeHttpRequest({
  url: `https://api.lighthouse.storage/api/lighthouse/deal_status?cid=${cid}`,
  timeout: 4000
});

let ipfsRes, lhRes;
try {
  [ipfsRes, lhRes] = await Promise.all([ipfsRequest, lighthouseRequest]);
} catch (e) {
  // 网络层面失败视为不可检索 (状态 2)
  return encodeResponse(2, 0);
}

// 判定逻辑
let status = 2; // 默认 Not Retrieveable
let actualEndTime = 0;

if (!ipfsRes.error && ipfsRes.status === 200) {
  status = 1; // 只要 IPFS 通了，至少是 Retrieveable

  // 尝试获取 Deal 详情
  if (!lhRes.error && Array.isArray(lhRes.data) && lhRes.data.length > 0) {
    const latestDealId = lhRes.data[lhRes.data.length - 1].DealID;

    const filfoxRes = await Functions.makeHttpRequest({
      url: `https://filfox.info/api/v1/deal/${latestDealId}`,
      timeout: 3000
    });

    if (!filfoxRes.error && filfoxRes.status === 200) {
      // 只要有有效的订单信息，即视为 Ensured
      status = 0; 
      actualEndTime = filfoxRes.data.endTimestamp;
    }
  }
}

// 如果状态不是 0，强制时间戳为 0
const finalTime = status === 0 ? actualEndTime : 0;

// 辅助函数：将两个数字手动 ABI 编码为 64 字节的 Uint8Array
function encodeResponse(s, t) {
  const result = new Uint8Array(64);
  const view = new DataView(result.buffer);
  // ABI encode uint256: 占用 32 字节，高位补零，低位写入数据
  view.setUint32(28, s); // 状态写入第一个 32 字节的末尾
  view.setUint32(60, t); // 时间戳写入第二个 32 字节的末尾
  return result;
}

return encodeResponse(status, finalTime);