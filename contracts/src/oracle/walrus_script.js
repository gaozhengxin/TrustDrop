// Walrus 主网 Epoch 0 开始时间: 2025-12-16T00:00:00Z
const initDate = new Date('2025-12-16T00:00:00Z');
const initEpoch = 20;
const epochLength = 1209600; // 2 weeks in seconds

const encodeResponse = (s, t) => {
  const res = new Uint8Array(64);
  const v = new DataView(res.buffer);
  v.setUint32(28, s); 
  v.setUint32(60, t); 
  return res;
};

const blobIdHex = args[0].startsWith('0x') ? args[0].slice(2) : args[0];
const apiKey = args[1];

// 手动实现 hex 到 base64url 的转换
const hexToBase64Url = (hex) => {
  const bytes = new Uint8Array(hex.match(/.{1,2}/g).map(byte => parseInt(byte, 16)));
  let binary = '';
  for (let i = 0; i < bytes.byteLength; i++) {
    binary += String.fromCharCode(bytes[i]);
  }
  return btoa(binary)
    .replace(/\+/g, '-')
    .replace(/\//g, '_')
    .replace(/=/g, '');
};

const base64Url = hexToBase64Url(blobIdHex);
// --------------------------

const startTimeSec = initDate.getTime() / 1000;
const currentTimeSec = Date.now() / 1000;
const currentEpoch = initEpoch + (currentTimeSec - startTimeSec) / epochLength;

try {
  const res = await Functions.makeHttpRequest({
    url: `https://api.blockberry.one/walrus-mainnet/v1/blobs/${base64Url}`,
    headers: { 'accept': '*/*', 'x-api-key': apiKey },
    timeout: 5000
  });

  if (res.error || res.status !== 200) return encodeResponse(0, 0);

  const { endEpoch } = res.data;
  const status = endEpoch > currentEpoch ? 2 : 1;
  const endTimestamp = Math.floor(startTimeSec + (endEpoch - initEpoch) * epochLength);

  return encodeResponse(status, endTimestamp);

} catch (e) {
  return encodeResponse(0, 0);
}