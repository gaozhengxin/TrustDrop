// Walrus 主网 Epoch 0 开始时间: 2025-12-16T00:00:00Z
const initDate = new Date('2025-12-16T00:00:00Z');
const initEpoch = 20;
const epochLength = 1209600; // 2 weeks in seconds

// 修正：将 status 和转换后的 Unix 时间戳打包返回
const encodeResponse = (s, t) => {
  const res = new Uint8Array(64);
  const v = new DataView(res.buffer);
  v.setUint32(28, s); 
  v.setUint32(60, t); // 这里现在存的是秒级时间戳
  return res;
};

const blobIdHex = args[0].startsWith('0x') ? args[0].slice(2) : args[0];
const apiKey = args[1];

const base64 = Buffer.from(blobIdHex, 'hex')
  .toString('base64')
  .replace(/\+/g, '-')
  .replace(/\//g, '_')
  .replace(/=/g, '');

const startTimeSec = initDate.getTime() / 1000;
const currentTimeSec = Date.now() / 1000;
const currentEpoch = initEpoch + (currentTimeSec - startTimeSec) / epochLength;

try {
  const res = await Functions.makeHttpRequest({
    url: `https://api.blockberry.one/walrus-mainnet/v1/blobs/${base64}`,
    headers: { 'accept': '*/*', 'x-api-key': apiKey },
    timeout: 5000
  });

  if (res.error || res.status !== 200) return encodeResponse(0, 0);

  const { endEpoch } = res.data;
  const status = endEpoch > currentEpoch ? 2 : 1;

  // 【核心修复】：将 endEpoch 转换为合约能识别的 Unix Timestamp
  // 公式：初始秒数 + (目标Epoch - 初始Epoch) * Epoch长度
  const endTimestamp = Math.floor(startTimeSec + (endEpoch - initEpoch) * epochLength);

  return encodeResponse(status, endTimestamp);

} catch (e) {
  return encodeResponse(0, 0);
}