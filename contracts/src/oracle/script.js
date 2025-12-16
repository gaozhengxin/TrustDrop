const mod = await import("https://cdn.skypack.dev/blake3-js");
//const Buffer = await import("node:buffer").then(m => m.Buffer);
const blake3 = mod.default;

const blob_id = args[0];
const min_epoch_to_live = args[1];
const blob_size = args[2];
const apiKey = secrets.apiKey;

if (!secrets.apiKey) {
    throw Error("Missing secret: apiKey");
}

// check blob metadata
const checkBlobMeta = async (blob_id, apiKey) => {
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
}

// check blob content
const checkBlobContent = async (blob_id) => {
    const walrus_aggregate_urls = [
        'https://walrus-main-aggregator.4everland.org',
        'https://sui-walrus-mainnet-aggregator.bwarelabs.com',
        'https://walrus.blockscope.net',
        'https://walrus-aggregator.brightlystake.com',
        'https://walrus-aggregator.chainbase.online',
        'https://walmain.agg.chainflow.io',
        'https://walrus-mainnet-aggregator.crouton.digital',
        'https://walrus-mainnet-aggregator.dzdaic.com',
        'https://walrus.globalstake.io',
        'https://aggregator.walrus-mainnet.h2o-nodes.com',
        'https://mainnet-aggregator.hoh.zone',
        'https://mainnet-walrus-aggregator.kiliglab.io'
    ];
    const index = Math.floor(Math.random() * walrus_aggregate_urls.length);
    const walrus_aggregate_url = walrus_aggregate_urls[index] + '/v1/blobs/' + blob_id;
    console.log(`aggregator: ${walrus_aggregate_url}`);

    let response;

    const options = {
        method: 'GET',
    };

    try {
        response = await fetch(walrus_aggregate_url, options);
    } catch (error) {
        throw (`Fetch Error: ${error.message}, aggregator index: ${index}`);
    }

    try {
        if (!response.ok) {
            const errorBody = await response.text();
            throw (`HTTP Error ${response.status}: ${errorBody.substring(0, 100)}`);
        }
        const buf = await response.arrayBuffer();
        const u8 = new Uint8Array(buf);

        console.log(`length: ${u8.length}`);
        //console.log(`data: ${Buffer.from(u8).toString('hex')}`);
        //return hashArrayBuffer(u8);
        return "1111";
    } catch (error) {
        throw (`Response Processing Error: ${error.message}`);
    }
}

const hashArrayBuffer = async (data) => {
    const hasher = blake3.newRegular();

    const CHUNK_SIZE = 64 * 1024; // 64KB

    for (let offset = 0; offset < data.length; offset += CHUNK_SIZE) {
        const end = Math.min(offset + CHUNK_SIZE, data.length);
        const chunk = data.subarray(offset, end);
        hasher.update(chunk);
    }

    const digest = hasher.finalize();
    return digest;
};

//await checkBlobMeta(blob_id, apiKey);

//const blob_hash = await checkBlobContent(blob_id);
//console.log(`blob_hash: ${blob_hash}`);

for (let i = 0; i < 3; i++) {
    const u8 = new Uint8Array(150 * 1024);
    u8.fill(1);
    const digest = hashArrayBuffer(u8);
    console.log(digest);
}

return (Functions.encodeString(blob_id));

/*
args[0] tba9dVjvTALBy_fVVBIfuY8PBCiJ5nWg15umBKYk8q4
args[1] 2
args[2] 2673336
secrets.apiKey secrets eNx0cS4PemfQtVaArXbRbHcyJTnP0l
*/

/*
args[0] TGBXYr0km3mLvyNHt6bQXbLHslBR5HAwV8mAQ-HFJIo
args[1] 2
args[2] 98
secrets.apiKey secrets eNx0cS4PemfQtVaArXbRbHcyJTnP0l
 */

/*
args[0] TGBXYr0km3mLvyNHt6bQXbLHslBR5HAwV8mAQ-HFJIo
args[1] 2
args[2] 98
secrets.apiKey secrets eNx0cS4PemfQtVaArXbRbHcyJTnP0l
 */

/*
args[0] kmuk_tJZj95vDq267D8UkUi6wnSZuCpEGAN6Dzmzycg
args[1] 2
args[2] 1782224
secrets.apiKey secrets eNx0cS4PemfQtVaArXbRbHcyJTnP0l
 */

/*
args[0] PF6yPILshumAGv6UD031LkXpZlkRk5KtX5PDyA-15og
args[1] 2
args[2] 1587
secrets.apiKey secrets eNx0cS4PemfQtVaArXbRbHcyJTnP0l
 */

/*
args[0] Na2pPYemtxFBvGTvFrjMynmuwvcK4fD86D6S2w1cfCI
args[1] 2
args[2] 1336668
secrets.apiKey secrets eNx0cS4PemfQtVaArXbRbHcyJTnP0l
*/

/*
args[0] g9tk2Gg1G91UWnklM9vIKusDvr4LZN1aFdl5FJBiYv4
args[1] 2
args[2] 
secrets.apiKey secrets eNx0cS4PemfQtVaArXbRbHcyJTnP0l
*/