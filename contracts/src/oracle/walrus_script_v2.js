const Buffer = await import("node:buffer").then(m => m.Buffer);

const blob_id_hex = args[0].startsWith("0x") ? args[0].slice(2) : args[0];
const blob_id_bytes = Buffer.from(blob_id_hex, 'hex');
let blob_id = blob_id_bytes.toString('base64');
blob_id = blob_id
    .replace(/\+/g, '-')
    .replace(/\//g, '_')
    .replace(/=/g, '');
console.log(`blob_id: ${blob_id}`);

const min_epoch_to_live = args[1];
const blob_size = args[2];
const apiKey = secrets.apiKey;


if (!secrets.apiKey) {
    throw Error("Missing secret: apiKey");
}

// check blob metadata

const options = {
    method: 'GET',
    headers: { accept: '*/*', 'x-api-key': apiKey }
};

const blockberry_url = 'https://api.blockberry.one/walrus-mainnet/v1/blobs/' + blob_id;
let response;
let result;

try {
    response = await fetch(blockberry_url, options);
} catch (error) {
    return Functions.encodeString(`Fetch Error: ${error.message}`);
}

try {
    if (!response.ok) {
        const errorBody = await response.text();
        return Functions.encodeString(`HTTP Error ${response.status}: ${errorBody.substring(0, 100)}`);
    }

    result = await response.json();

} catch (error) {
    return Functions.encodeString(`Response Processing Error: ${error.message}`);
}

const { startEpoch, endEpoch, size } = result;
console.log(`${JSON.stringify({ startEpoch, endEpoch, size }, undefined, 2)}`);

const epochLength = 2 * 7 * 86400;

const initDate = new Date('2025-12-16T00:00:00Z');
const initEpoch = 20;

const currentEpoch = initEpoch + (Date.now() / 1000 - initDate.getTime() / 1000) / epochLength;

console.log(`currentEpoch ${currentEpoch}`);

if (currentEpoch < startEpoch) {
    throw ("Premature access: Start epoch not yet reached.");
}

if (currentEpoch > endEpoch) {
    throw ("Access expired: Past the end epoch.");
}

if (endEpoch - startEpoch < min_epoch_to_live) {
    throw ("Availability duration too short.");
}

if (blob_size != size) {
    throw ("Blob size not match.");
}

return (Functions.encodeString(blob_id_hex));

/*
args[0] 4c605762bd249b798bbf2347b7a6d05db2c7b25051e4703057c98043e1c5248a // TGBXYr0km3mLvyNHt6bQXbLHslBR5HAwV8mAQ-HFJIo
args[1] 2
args[2] 98
secrets.apiKey secrets eNx0cS4PemfQtVaArXbRbHcyJTnP0l
 */
