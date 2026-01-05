// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

// src/interfaces/IOracleClient.sol
interface IOracleClient {
    function onSuccess(bytes calldata cCipher) external;
    function onFail(bytes calldata cCipher) external;
}

// src/interfaces/IOracleProxy.sol
interface IOracleProxy {
    function request(bytes memory c_cipher) external;
}

// src/interfaces/ITradeHub.sol

interface ITradeHub {
    function reportListEvent(bytes32 saleId, bytes memory dataCommitment, uint256 price, uint256 version) external;
    function reportUpdateEvent(bytes32 saleId, bytes memory dataCommitment, uint256 newPrice, uint256 version) external;
    function reportDelistEvent(bytes32 saleId) external;
    function reportPurchaseEvent(bytes32 saleId, bytes memory dataCommitment, address buyer, uint256 price) external;
    function reportSettleEvent(address buyer, bytes32 saleId, bytes memory dataCommitment) external;
    function reportRefundEvent(address buyer, bytes32 saleId, bytes memory dataCommitment, uint256 amount) external;
}

// src/interfaces/IVerifier.sol
interface IVerifier {
    function verifyVSS(
        bytes calldata proof,
        bytes calldata publicValues
    ) external returns (bool);
    function verifyVDD(
        bytes calldata proof,
        bytes calldata publicValues
    ) external returns (bool);
}

// src/lib/Types.sol
library Types {
    enum HashType {
        SHA256, // 0
        BLAKE2B // 1
    }

    enum SynmetricKeyType {
        CHACHA8 // 0
    }

    enum PubkeyType {
        SECP256K1_COMPRESSED,
        ED25519
    }

    enum DataCommitmentType {
        WALRUS_BLOB_ID,
        CID
    }

    type Hash is bytes32;

    struct Pubkey {
        bytes data;
    }

    type SynmetricKey is bytes32;

    type Cipher32 is bytes32;

    struct DataCommitment {
        bytes data;
    }

    function eq(Hash a, Hash b) internal pure returns (bool) {
        return Hash.unwrap(a) == Hash.unwrap(b);
    }

    function neq(Hash a, Hash b) internal pure returns (bool) {
        return Hash.unwrap(a) != Hash.unwrap(b);
    }

    function toHash(bytes32 b) internal pure returns (Hash) {
        return Hash.wrap(b);
    }
    function toCipher32(bytes32 b) internal pure returns (Cipher32) {
        return Cipher32.wrap(b);
    }
    function toDataCommitment(
        bytes memory b
    ) internal pure returns (DataCommitment memory r) {
        require(b.length == 36, "Invalid CID length");
        assembly {
            r := mload(0x40)
            mstore(0x40, add(r, 32))
            mstore(r, b)
        }
    }
}

// src/VSS.sol

contract VSS {
    using Types for *;

    struct AudienceInfo {
        Types.Hash vssKeyCommitment;
        Types.Cipher32 encryptedVssKey;
    }

    // --- zk 验证器 ---
    IVerifier public verifier;

    // --- 状态变量 ---
    address public owner;
    Types.Pubkey public ownerPublicKey;
    Types.Hash public dataKeyCommitment;

    // 全局版本号：每次 shareDataKey 时自增
    uint256 public version;

    // 位图：每个 bit 代表一个观众是否在当前 version 中被授权
    // bit n 为 1 表示 index 为 n 的观众有权限
    uint256 public privyBitmap;

    // 最大 256 人，索引 0-255
    AudienceInfo[256] public audienceList;
    mapping(address => uint256) public audienceIndex;
    // 记录地址是否已注册，区分 index 0 和未注册
    mapping(address => bool) public isRegistered;
    uint256 public nextAudienceIndex = 0;

    // --- 事件 ---
    event Joined(address indexed user, uint256 index);
    event VssKeyUpdated(address indexed user, Types.Hash newVssKeyCommitment);
    event DataKeyShared(uint256 newPrivyBitmap, uint256 newVersion);
    event DataKeyCommitmentUpdated(Types.Hash newCommitment);
    event OwnershipTransferred(
        address indexed previousOwner,
        address indexed newOwner
    );

    modifier onlyOwner() {
        require(msg.sender == owner, "Not owner");
        _;
    }

    constructor(Types.Pubkey memory _ownerPubKey) {
        owner = msg.sender;
        ownerPublicKey = _ownerPubKey;
    }

    // --- 内部逻辑 ---

    function _addAudience(
        address user,
        Types.Hash vssKeyCommitment,
        Types.Cipher32 encryptedVssKey
    ) internal {
        require(nextAudienceIndex < 256, "Audience list full");
        require(!isRegistered[user], "Audience exists");

        uint256 idx = nextAudienceIndex++;
        audienceIndex[user] = idx;
        isRegistered[user] = true;

        audienceList[idx] = AudienceInfo({
            vssKeyCommitment: vssKeyCommitment,
            encryptedVssKey: encryptedVssKey
        });

        emit Joined(user, idx);
    }

    // --- 外部接口 ---

    function join(
        Types.Hash vssKeyCommitment,
        Types.Cipher32 encryptedVssKey
    ) external {
        _addAudience(msg.sender, vssKeyCommitment, encryptedVssKey);
    }

    function updateVssKey(
        Types.Hash vssKeyCommitment,
        Types.Cipher32 encryptedVssKey
    ) public {
        _updateVssKeyInternal(msg.sender, vssKeyCommitment, encryptedVssKey);
    }

    function _updateVssKeyInternal(
        address user,
        Types.Hash vssKeyCommitment,
        Types.Cipher32 encryptedVssKey
    ) internal {
        _revokePrivyInternal(user);
        uint256 idx = audienceIndex[user];

        audienceList[idx].vssKeyCommitment = vssKeyCommitment;
        audienceList[idx].encryptedVssKey = encryptedVssKey;

        emit VssKeyUpdated(user, vssKeyCommitment);
    }

    function isPrivy(address user) public view returns (bool) {
        if (version == 0 || !isRegistered[user]) return false;

        uint256 idx = audienceIndex[user];
        return (privyBitmap & (uint256(1) << idx)) != 0;
    }

    function _revokePrivyInternal(address user) internal {
        require(isRegistered[user], "Not an audience");
        uint256 idx = audienceIndex[user];

        privyBitmap &= ~(uint256(1) << idx);
    }

    function submitDataKeyCommitment(Types.Hash _commitment) public onlyOwner {
        dataKeyCommitment = _commitment;
        emit DataKeyCommitmentUpdated(_commitment);
    }

    /**
     * @notice 分发数据密钥并同步更新位图
     * @param audiences 此次获得授权的观众地址列表
     * @param encryptedDataKeys 对应的加密数据密钥
     */
    function shareDataKey(
        bytes calldata proof,
        bytes calldata publicValues,
        address[] memory audiences,
        Types.Cipher32[] memory encryptedDataKeys
    ) public onlyOwner {
        require(
            audiences.length == encryptedDataKeys.length,
            "Mismatched input"
        );

        // TODO verify public values
        // 1. origin msg hash
        // 2. vssKeyCommitment
        // 3. encryptedDataKeys
        require(
            verifier.verifyVSS(proof, publicValues),
            "VSS verification failed"
        );

        version += 1;

        uint256 updatedBitmap = privyBitmap;

        for (uint256 i = 0; i < audiences.length; i++) {
            address user = audiences[i];

            if (isRegistered[user]) {
                uint256 idx = audienceIndex[user];
                updatedBitmap |= (uint256(1) << idx);
            }
        }

        privyBitmap = updatedBitmap;

        emit DataKeyShared(updatedBitmap, version);
    }

    function transferOwner(address newOwner) public onlyOwner {
        require(newOwner != address(0), "Invalid address");
        emit OwnershipTransferred(owner, newOwner);
        owner = newOwner;
    }
}

// src/VDD.sol

contract VDD is VSS, IOracleClient {
    using Types for *;

    IOracleProxy public oracleWrapper;

    struct DataInfo {
        Types.DataCommitment commitment;
        uint256 size;
        string info;
        uint256 timestamp;
    }

    // 使用 commitment 的哈希值作为 key
    mapping(bytes32 => DataInfo) public dataInfoList;

    // State 1: vddVerified[cCipher] = true means ZK proof passed
    mapping(bytes => bool) public vddVerified;

    // State 2: oracleSuccessUntil[cCipher] = timestamp.
    mapping(bytes => uint256) public oracleSuccessUntil;

    event DataListed(bytes32 indexed dataId, uint256 size);
    event VDDProofSubmitted(bytes cCipher);

    constructor(
        Types.Pubkey memory _ownerPubKey,
        address _oracleWrapper
    ) VSS(_ownerPubKey) {
        oracleWrapper = IOracleProxy(_oracleWrapper);
    }

    // 根据 commitment 原始字节查询
    function retrieveDataInfo(
        bytes memory commitment
    ) public view returns (DataInfo memory) {
        bytes32 dataId = keccak256(commitment);
        return dataInfoList[dataId];
    }

    // 由 Owner 上架数据元信息
    function listDataInfo(
        Types.DataCommitment memory _commitment,
        uint256 _size,
        string memory _info
    ) public onlyOwner {
        bytes32 dataId = keccak256(_commitment.data);
        dataInfoList[dataId] = DataInfo({
            commitment: _commitment,
            size: _size,
            info: _info,
            timestamp: block.timestamp
        });
        emit DataListed(dataId, _size);
    }

    // 提交 VDD 证明并触发 Oracle 检查
    function submitVDDProof(
        bytes calldata proof,
        bytes calldata publicValues,
        bytes calldata dataCommitment,
        bytes memory cCipher // 加密后的密文，用于 Oracle 校验存储节点
    ) public onlyOwner {
        // 1. ZK Verification
        // TODO verify public values
        // 1. data commitment
        // 2. dataKeyCommitment
        // 3. cCipher
        require(verifier.verifyVDD(proof, publicValues), "VDD verification failed");

        vddVerified[cCipher] = true;

        oracleWrapper.request(cCipher);
        emit VDDProofSubmitted(cCipher);
    }

    function triggerOracle(bytes memory cCipher) public onlyOwner {
        require(vddVerified[cCipher], "VDD not verified");
        oracleWrapper.request(cCipher);
    }

    // Oracle 异步回调：验证成功
    function onSuccess(bytes calldata cCipher) external virtual {
        require(msg.sender == address(oracleWrapper), "Only oracle proxy");
        if (!vddVerified[cCipher]) {
            return;
        }
        // TODO let oracle pass in actual life span
        oracleSuccessUntil[cCipher] = block.timestamp + 30 days;
    }

    // Oracle 异步回调：验证失败
    function onFail(bytes calldata cCipher) external virtual {
        require(msg.sender == address(oracleWrapper), "Only oracle proxy");
        if (!vddVerified[cCipher]) {
            return;
        }
        oracleSuccessUntil[cCipher] = 0;
    }
}

// src/TradeChannel.sol

// Encapsulated parameters for ZK verification
struct VSSArgs {
    Types.Cipher32 encryptedDataKey;
    bytes proof;
    bytes publicValues;
}

struct VDDArgs {
    bytes proof;
    bytes publicValues;
    bytes cCipher;
}

struct TradeInfo {
    bytes32 saleDigest;
    uint256 price;
    uint256 deadline;
    bytes dataCommitment;
    Types.Hash vssKeyCommitment;
}

abstract contract ReentrancyGuard {
    uint256 private constant _NOT_ENTERED = 1;
    uint256 private constant _ENTERED = 2;

    uint256 private _status = _NOT_ENTERED;

    modifier nonReentrant() {
        require(_status == _NOT_ENTERED, "ReentrancyGuard: reentrant call");
        _status = _ENTERED;
        _;
        _status = _NOT_ENTERED;
    }
}

contract TradeChannel is VDD, ReentrancyGuard {
    using Types for *;

    uint256 public constant LIVING_WINDOW = 7 days;

    ITradeHub public hub;
    mapping(bytes32 => uint256) public saleVersions;
    mapping(bytes32 => bool) public pendingTrades;
    mapping(address => uint256) public lockedBalances;

    constructor(
        Types.Pubkey memory _ownerPubKey,
        address _oracleWrapper,
        address _hub
    ) VDD(_ownerPubKey, _oracleWrapper) {
        hub = ITradeHub(_hub);
    }

    function getTradeDigest(
        address buyer,
        TradeInfo memory info,
        uint256 version
    ) public pure returns (bytes32) {
        return
            keccak256(
                abi.encode(
                    buyer,
                    info.saleDigest,
                    version,
                    info.price,
                    info.deadline,
                    info.dataCommitment,
                    info.vssKeyCommitment
                )
            );
    }

    // --- Actions ---

    function purchase(
        bytes32 saleId,
        uint256 version,
        uint256 price,
        uint256 deadline,
        bytes calldata dataCommitment,
        Types.Hash vssKeyCommitment,
        Types.Cipher32 encryptedVssKey
    ) external payable {
        require(version == saleVersions[saleId], "Wrong version");
        require(msg.value == price, "Exact price required");

        TradeInfo memory info = TradeInfo(
            saleId,
            price,
            deadline,
            dataCommitment,
            vssKeyCommitment
        );
        bytes32 digest = getTradeDigest(msg.sender, info, version);
        require(!pendingTrades[digest], "Trade already pending");
        pendingTrades[digest] = true;
        lockedBalances[msg.sender] += msg.value;

        if (!isRegistered[msg.sender]) {
            _addAudience(msg.sender, vssKeyCommitment, encryptedVssKey);
        } else {
            // Only update if commitment changed
            if (
                audienceList[audienceIndex[msg.sender]].vssKeyCommitment.neq(
                    vssKeyCommitment
                )
            ) {
                _updateVssKeyInternal(
                    msg.sender,
                    vssKeyCommitment,
                    encryptedVssKey
                );
            }
        }
        hub.reportPurchaseEvent(saleId, dataCommitment, msg.sender, price);
    }

    /**
     * @notice Seller fulfills requirements for a buyer.
     */
    function fulfill(
        address buyer,
        TradeInfo calldata info,
        uint256 saleVersion,
        VSSArgs calldata vss,
        VDDArgs calldata vdd
    ) external onlyOwner {
        bytes32 digest = getTradeDigest(buyer, info, saleVersion);
        require(pendingTrades[digest], "No trade");

        // 1. Skip VSS if already privy
        if (!isPrivy(buyer)) {
            address[] memory singleAudience = new address[](1);
            singleAudience[0] = buyer;
            Types.Cipher32[] memory singleKey = new Types.Cipher32[](1);
            singleKey[0] = vss.encryptedDataKey;
            shareDataKey(
                vss.proof,
                vss.publicValues,
                singleAudience,
                singleKey
            );
        }

        // 2. Skip VDD/Oracle if already valid and not expired
        if (!vddVerified[vdd.cCipher]) {
            submitVDDProof(
                vdd.proof,
                vdd.publicValues,
                info.dataCommitment,
                vdd.cCipher
            );
        } else if (
            oracleSuccessUntil[vdd.cCipher] <= block.timestamp + LIVING_WINDOW
        ) {
            triggerOracle(vdd.cCipher);
        }
    }

    /**
     * @notice Settlement: Can be called by anyone once requirements are met.
     */
    function settle(
        address buyer,
        TradeInfo calldata info,
        uint256 saleVersion,
        bytes calldata cCipher
    ) external {
        bytes32 digest = getTradeDigest(buyer, info, saleVersion);
        require(pendingTrades[digest], "No trade");

        // Conditions for settlement:
        // 1. Buyer has keys (VSS Privy)
        require(isPrivy(buyer), "Buyer not privy");

        // 2. Data accessibility is confirmed and currently valid
        require(
            oracleSuccessUntil[cCipher] > block.timestamp,
            "Oracle proof expired or missing"
        );

        // 3. Optional: Logic to allow settlement if 30 days passed since deadline (as backup)
        // require(block.timestamp > info.deadline + 30 days || oracleSuccessUntil[cCipher] > 0, "Wait");

        delete pendingTrades[digest];
        require(
            lockedBalances[buyer] >= info.price,
            "Insufficient locked balance"
        );
        lockedBalances[buyer] -= info.price;

        _revokePrivyInternal(buyer);

        payable(owner).transfer(info.price);
        hub.reportSettleEvent(buyer, info.saleDigest, info.dataCommitment);
    }

    function refund(
        address buyer,
        TradeInfo calldata info,
        uint256 version
    ) external nonReentrant {
        bytes32 digest = getTradeDigest(buyer, info, version);
        require(pendingTrades[digest], "No trade");
        require(block.timestamp > info.deadline, "Not expired");

        delete pendingTrades[digest];
        require(
            lockedBalances[buyer] >= info.price,
            "Insufficient locked balance"
        );
        lockedBalances[buyer] -= info.price;

        payable(buyer).transfer(info.price);
        hub.reportRefundEvent(
            buyer,
            info.saleDigest,
            info.dataCommitment,
            info.price
        );
    }
}

