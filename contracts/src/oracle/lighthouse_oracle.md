## Chainlink function setting
https://docs.chain.link/chainlink-functions/supported-networks#arbitrum-sepolia-testnet

## Contract demo
https://remix.ethereum.org/#url=https://docs.chain.link/samples/ChainlinkFunctions/GettingStartedFunctionsConsumer.sol

## API demo
### 查文件
`bafkreidn5zqnpzy4kfdv3xilsucbzvube726wly5g2zmi2do2kvlag3fr4` = `0x6261666B726569646E357A716E707A79346B6664763378696C737563627A76756265373236776C793567327A6D6932646F326B766C616733667234`
```
curl -H "Accept: application/vnd.ipld.dag-json" "https://ipfs.io/ipfs/bafkreidn5zqnpzy4kfdv3xilsucbzvube726wly5g2zmi2do2kvlag3fr4?format=dag-json"
```
```
{"/":{"bytes":"WyLmnJ3ovp7nmb3luJ3lvankupHpl7QiLCLljYPph4zmsZ/pmbXkuIDml6Xov5giLCLkuKTlsrjnjL/lo7DllbzkuI3kvY8iLCLovbvoiJ/lt7Lov4fkuIfph43lsbEiXQo"}}
```
或
`bafybeiha7o755i4l36ppzgy3mvslf4757lmw4wgcgdtlffyhxohyqz5cmq` = `0x626166796265696861376F37353569346C333670707A6779336D76736C66343735376C6D77347767636764746C66667968786F6879717A35636D71`
```
curl -H "Accept: application/vnd.ipld.dag-json" \
"https://ipfs.io/ipfs/bafybeiha7o755i4l36ppzgy3mvslf4757lmw4wgcgdtlffyhxohyqz5cmq?format=dag-json"
```
```
{"Data":{"/":{"bytes":"CAIYiaCEASCAgBAggIAQIICAECCAgBAggIAQIICAECCAgBAggIAQIImgBA"}},"Links":[{"Hash":{"/":"bafkreihfasxwimlhox53w6ufl7gnqceelxivm5jepveprte7fqjybms4ry"},"Name":"","Tsize":262144},{"Hash":{"/":"bafkreia7jdx3pzt3hti5ukptbu4k63qnwhpropewprapqrsk6uhoi5ze3u"},"Name":"","Tsize":262144},{"Hash":{"/":"bafkreihkwtq67cotjxmdk33etktfwapgloqmcopvb4l7grecuwz2g7ts6e"},"Name":"","Tsize":262144},{"Hash":{"/":"bafkreickba25zt37ktnz3v647hiwrwzzc4ibarghbcsmkpaa2ydimse2oi"},"Name":"","Tsize":262144},{"Hash":{"/":"bafkreia4usgughbl7cic2kaemdsrwkkf7zathy4vtnrdgkfajcpmxgjv2y"},"Name":"","Tsize":262144},{"Hash":{"/":"bafkreibwmdve2c2ltdthxrqvtacfljyelltslffr4sebprb5tmfkaxqj3u"},"Name":"","Tsize":262144},{"Hash":{"/":"bafkreih7r66tholmxk2kkodkgielm7lfoin3rjgpoblpybzuxpiuuogzku"},"Name":"","Tsize":262144},{"Hash":{"/":"bafkreifcljypvfk766ubgdgn4uxnswladu5bsxq2k6zu4zdutphr2342vy"},"Name":"","Tsize":262144},{"Hash":{"/":"bafkreihapoarc6itasyxqxtresuktjxzraduzqe4h6oxgvtf7qloaj455u"},"Name":"","Tsize":69641}]}
```

### 查订单
```
curl 'https://api.lighthouse.storage/api/lighthouse/deal_status?cid=bafybeiha7o755i4l36ppzgy3mvslf4757lmw4wgcgdtlffyhxohyqz5cmq'
```
```
[
    {
        "DealID": 131800583,
        "Provider": 8403
    },
    {
        "DealID": 131799904,
        "Provider": 10479
    },
    {
        "DealID": 131799841,
        "Provider": 3623017
    }
]
```

### 查看订单信息
```
curl -X GET "https://filfox.info/api/v1/deal/131800583"
```
```
{"id":131800583,"height":5616010,"timestamp":1766786700,"message":"bafy2bzaceal4fuzbbolgt3gpwp37rpfe3xxg7tt6iyyd7r4ings5hc7iwcvv4","pieceCid":"baga6ea4seaqhoqqqn44yekan2aywyn6ltln7ll3y6ddahwzr2borofv3uc3suaa","pieceSize":34359738368,"verifiedDeal":true,"client":"f1ggmci7w2weizhh36uqetihmh76ewgme6hwgowti","provider":"f08403","providerTag":{"name":"TippyFlits","signed":true},"startEpoch":5624538,"startTimestamp":1767042540,"endEpoch":7150938,"endTimestamp":1812834540,"storagePricePerEpoch":"0","stroagePrice":"0","clientCollateral":"0","providerCollateral":"3122484984966874"}
```

## 合约地址
### Arbitrum sepolia
| Contract | Address |
| - | - |
| Sub id | 550 |
| Lighthouse functions consumer | 0x2b2a0BcAbEc08c394F438E353a2eD8DD510067d8 |
| Oracle proxy | 0x0c217d334734A9aF7f51FCfd33Eaad616b312f35 |
| Client example | 0x6Bb1472e43BaBff76A9391ef725F192bF23cD7fF |


/Users/xiexie/playDM/prover/lighthouseOracle/lighthouseOracle/lib/@chainlink/contracts@1.5.0/src/v0.8/functions/v1_0_0/FunctionsClient.sol

/Users/xiexie/playDM/prover/lighthouseOracle/lighthouseOracle/lib/@chainlink/contracts@1.5.0/src/contracts@1.5.0/src/v0.8/functions/v1_0_0/FunctionsClient.sol