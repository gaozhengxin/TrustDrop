// -------------------- Chainlink Functions Script --------------------

// 1. 硬编码配置参数
// Walrus 主网 Epoch 0 开始时间: 2025-12-16T00:00:00Z
// 初始 Epoch 为 20，每个 Epoch 长度为 2 周 (1209600 秒)
const initDate = new Date('2025-12-16T00:00:00Z');
const initEpoch = 20;
const epochLength = 2 * 7 * 86400; // 1,209,600 seconds

// 2. 定义响应编码函数
// 将状态 (s) 和过期 Epoch (t) 打包成 64 字节的 Uint8Array
// 对应 Solidity 的 bytes 类型，前32字节存状态，后32字节存过期时间
const encodeResponse = (s, t) => {
  const res = new Uint8Array(64);
  const v = new DataView(res.buffer);
  // 将状态 s 放入前 32 字节的末尾 (大端序)
  v.setUint32(28, s);
  // 将时间戳 t 放入后 32 字节的末尾 (大端序)
  v.setUint32(60, t);
  return res;
};

// 3. 获取传入参数
// args[0]: Blob ID (支持带 0x 前缀或不带的十六进制字符串)
const blobIdHex = args[0].startsWith('0x') ? args[0].slice(2) : args[0];
// args[1]: Walrus API Key (明文传递)
const apiKey = args[1];

// 4. 将 Hex Blob ID 转换为 Walrus 标准的 Base64URL 格式
const base64 = Buffer.from(blobIdHex, 'hex')
  .toString('base64')
  .replace(/\+/g, '-')  // '+' -> '-'
  .replace(/\//g, '_')  // '/' -> '_'
  .replace(/=/g, '');   // 移除末尾的填充 '='

// 5. 计算当前 Epoch
const currentEpoch = initEpoch + (Date.now() / 1000 - initDate.getTime() / 1000) / epochLength;

try {
  // 6. 发起 HTTP GET 请求查询 Blob 元数据
  const res = await Functions.makeHttpRequest({
    url: `https://api.blockberry.one/walrus-mainnet/v1/blobs/${base64}`,
    headers: {
      'accept': '*/*',
      'x-api-key': apiKey // 在请求头中携带 API Key
    },
    timeout: 5000 // 设置超时为 5 秒
  });

  // 7. 错误处理：网络错误或 API 返回非 200 状态码（例如 404 Not Found）
  // 返回状态 0 (查询不到)
  if (res.error || res.status !== 200) {
    return encodeResponse(0, 0);
  }

  // 8. 解析响应数据
  const { endEpoch } = res.data;

  // 9. 判定逻辑状态
  // 状态 2: endEpoch 严格大于当前 Epoch（符合“存活两周”的要求）
  // 状态 1: Blob 存在，但已处于最后一个 Epoch 或已过期
  const status = endEpoch > currentEpoch ? 2 : 1;

  // 10. 返回结果
  return encodeResponse(status, endEpoch);

} catch (e) {
  // 11. 捕获任何其他异常，返回默认错误状态 0
  return encodeResponse(0, 0);
}