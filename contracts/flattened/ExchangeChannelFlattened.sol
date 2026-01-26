// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.0 ^0.8.13;

// src/interfaces/IOracleClient.sol

interface IOracleClient {
    function onResponse(bytes memory cCipher, bytes memory response) external;
}

// src/interfaces/IOracleProxy.sol

interface IOracleProxy {
    function request(bytes memory c_cipher, address callback) external;
    function setWhitelist(address caller, bool allowed) external;
}

// src/interfaces/IVerifier.sol

interface IVSSVerifier {
    function verifyVSS(
        bytes calldata proof,
        bytes calldata publicValues,
        bytes32 bindingHash
    ) external returns (bool);
}

interface IVDDVerifier {
    function verifyVDD(
        bytes calldata proof,
        bytes calldata publicValues,
        bytes32 bindingHash
    ) external returns (bool);
}

contract MockVerifier is IVSSVerifier, IVDDVerifier {
    function verifyVSS(
        bytes calldata proof,
        bytes calldata publicValues,
        bytes32 bindingHash
    ) external returns (bool) {
        return true;
    }
    function verifyVDD(
        bytes calldata proof,
        bytes calldata publicValues,
        bytes32 bindingHash
    ) external returns (bool) {
        return true;
    }
}

// src/lib/Ownable.sol

abstract contract Ownable {
    address public owner;
    address public pendingOwner;

    event OwnershipTransferStarted(
        address indexed previousOwner,
        address indexed newOwner
    );
    event OwnershipTransferred(
        address indexed previousOwner,
        address indexed newOwner
    );
    event OwnershipTransferCanceled(address indexed pendingOwner);

    constructor(address _owner) {
        owner = _owner;
    }

    modifier onlyOwner() {
        require(msg.sender == owner, "Not owner");
        _;
    }

    function transferOwner(address newOwner) public virtual onlyOwner {
        require(newOwner != address(0), "Invalid address");
        require(newOwner != owner, "Already owner");

        pendingOwner = newOwner;
        emit OwnershipTransferStarted(owner, newOwner);
    }

    function cancelTransfer() public virtual onlyOwner {
        require(pendingOwner != address(0), "No pending transfer");

        emit OwnershipTransferCanceled(pendingOwner);
        pendingOwner = address(0);
    }

    function claimOwnership() public virtual {
        require(msg.sender == pendingOwner, "Not the pending owner");

        emit OwnershipTransferred(owner, pendingOwner);
        owner = pendingOwner;
        pendingOwner = address(0); // 清空状态
    }
}

// src/lib/ReentrancyGuard.sol

contract ReentrancyGuard {
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

    function unwrap(Hash _hash) internal pure returns (bytes32) {
        return Hash.unwrap(_hash);
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

// src/lib/Pausable.sol

abstract contract Pausable is Ownable {
    bool public paused;

    event Paused(address account);
    event Unpaused(address account);

    modifier whenNotPaused() {
        require(!paused, "Pausable: paused");
        _;
    }

    modifier whenPaused() {
        require(paused, "Pausable: not paused");
        _;
    }

    function pause() external onlyOwner whenNotPaused {
        paused = true;
        emit Paused(msg.sender);
    }

    function unpause() external onlyOwner whenPaused {
        paused = false;
        emit Unpaused(msg.sender);
    }
}

// src/VSS.sol

contract VSS is Pausable {
    using Types for *;

    struct AudienceInfo {
        Types.Hash vssKeyCommitment;
        Types.Cipher32 encryptedVssKey;
    }

    // --- 常量 ---
    uint256 public constant BUCKET_SIZE = 256;

    // --- 状态变量 ---
    IVSSVerifier public vssVerifier;
    Types.Pubkey public ownerPublicKey;
    Types.Hash public dataKeyCommitment;

    // 核心重构：从单 uint256 扩展为映射：bucketId => bitmap
    mapping(uint256 => uint256) public privyBitmaps;

    // 索引管理
    AudienceInfo[] public audienceList;
    mapping(address => uint256) public audienceIndex;
    mapping(address => bool) public isRegistered;

    // --- 事件 ---
    event Joined(address indexed user, uint256 index);
    event DataKeyShared(address[] audiences, Types.Cipher32[] encryptedDataKeys);
    event DataKeyCommitmentUpdated(Types.Hash newCommitment);

    constructor(
        Types.Pubkey memory _ownerPubKey,
        address owner,
        address _vssVerifier
    ) Ownable(owner) {
        ownerPublicKey = _ownerPubKey;
        vssVerifier = IVSSVerifier(_vssVerifier);
    }

    function init_VSS(
        Types.Pubkey memory _ownerPubKey,
        address owner,
        address _vssVerifier
    ) internal {
        ownerPublicKey = _ownerPubKey;
        vssVerifier = IVSSVerifier(_vssVerifier);
    }

    // --- 内部逻辑 ---

    function _addAudience(
        address user,
        Types.Hash vssKeyCommitment,
        Types.Cipher32 encryptedVssKey
    ) internal {
        require(!isRegistered[user], "Audience exists");

        uint256 idx = audienceList.length;
        audienceIndex[user] = idx;
        isRegistered[user] = true;

        audienceList.push(
            AudienceInfo({
                vssKeyCommitment: vssKeyCommitment,
                encryptedVssKey: encryptedVssKey
            })
        );

        emit Joined(user, idx);
    }

    // --- 外部接口 ---

    function join(
        Types.Hash vssKeyCommitment,
        Types.Cipher32 encryptedVssKey
    ) external virtual whenNotPaused {
        _addAudience(msg.sender, vssKeyCommitment, encryptedVssKey);
    }

    function isPrivy(address user) public view returns (bool) {
        if (!isRegistered[user]) return false;

        uint256 idx = audienceIndex[user];
        uint256 bucketId = idx / BUCKET_SIZE;
        uint256 offset = idx % BUCKET_SIZE;

        return (privyBitmaps[bucketId] & (uint256(1) << offset)) != 0;
    }

    function submitDataKeyCommitment(Types.Hash _commitment) public onlyOwner {
        if (Types.Hash.unwrap(dataKeyCommitment) != bytes32(0)) {
            revert("Cannot submit data key commitment again");
        }
        dataKeyCommitment = _commitment;
        emit DataKeyCommitmentUpdated(_commitment);
    }

    /**
     * @notice 分发数据密钥并同步更新位图
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

        bytes32[] memory pubkeys = new bytes32[](audiences.length);
        for (uint256 i = 0; i < audiences.length; i++) {
            require(isRegistered[audiences[i]], "Unregistered");
            pubkeys[i] = audienceList[audienceIndex[audiences[i]]].vssKeyCommitment.unwrap();
        }

        bytes32 bindingHash = keccak256(
            abi.encode(dataKeyCommitment, pubkeys, encryptedDataKeys)
        );

        require(
            vssVerifier.verifyVSS(proof, publicValues, bindingHash),
            "VSS verification failed"
        );

        for (uint256 i = 0; i < audiences.length; i++) {
            address user = audiences[i];
            if (isRegistered[user]) {
                uint256 idx = audienceIndex[user];
                uint256 bucketId = idx / BUCKET_SIZE;
                uint256 offset = idx % BUCKET_SIZE;

                privyBitmaps[bucketId] |= (uint256(1) << offset);
            }
        }

        emit DataKeyShared(audiences, encryptedDataKeys);
    }
}

// src/VDD.sol

contract VDD is VSS, IOracleClient {
    using Types for *;

    IOracleProxy public oracleWrapper;

    struct DataInfo {
        Types.DataCommitment commitment;
        uint256 timestamp;
    }

    // --- zk 验证器 ---
    IVDDVerifier public vddVerifier;

    // 使用 commitment 的哈希值作为 key
    mapping(bytes32 => DataInfo) public dataInfoList;

    // State 1: vddVerified[cCipher] = true means ZK proof passed
    mapping(bytes => bool) public vddVerified;

    // State 2: oracleSuccessUntil[cCipher] = timestamp.
    mapping(bytes => uint256) public oracleSuccessUntil;

    uint256 public immutable GRACE_PERIOD = 1 days;

    // lastOracleRequestAt[cCipher] = timestamp
    mapping(bytes => uint256) public lastOracleRequestAt;

    mapping(bytes32 => uint256) public dataReferenceCount;

    uint256 public constant ORACLE_COOLDOWN = 1 minutes;

    event DataListed(bytes32 indexed dataId);
    event DataDelisted(bytes32 indexed dataId);
    event VDDProofSubmitted(bytes cCipher);
    event OracleRequestSkipped(bytes cCipher, string msg);

    constructor(
        Types.Pubkey memory _ownerPubKey,
        address _oracleWrapper,
        address _owner,
        address _vssVerifier,
        address _vddVerifier
    ) VSS(_ownerPubKey, _owner, _vssVerifier) {
        oracleWrapper = IOracleProxy(_oracleWrapper);
        vddVerifier = IVDDVerifier(_vddVerifier);
    }

    function init_VDD(
        Types.Pubkey memory _ownerPubKey,
        address _oracleWrapper,
        address _owner,
        address _vssVerifier,
        address _vddVerifier
    ) internal {
        init_VSS(_ownerPubKey, _owner, _vssVerifier);
        oracleWrapper = IOracleProxy(_oracleWrapper);
        vddVerifier = IVDDVerifier(_vddVerifier);
    }

    function getDataId(
        bytes memory dataCommitment
    ) public pure returns (bytes32) {
        return keccak256(dataCommitment);
    }

    // 根据 commitment 原始字节查询
    function retrieveDataInfoById(
        bytes memory commitment
    ) public view returns (DataInfo memory) {
        bytes32 dataId = getDataId(commitment);
        return dataInfoList[dataId];
    }

    // 由 Owner 上架数据元信息
    function listDataInfo(
        Types.DataCommitment memory _commitment
    ) public onlyOwner returns (bytes32) {
        return _listDataInfo(_commitment);
    }

    function _listDataInfo(
        Types.DataCommitment memory _commitment
    ) internal whenNotPaused returns (bytes32) {
        bytes32 dataId = getDataId(_commitment.data);
        if (dataReferenceCount[dataId] == 0) {
            dataInfoList[dataId] = DataInfo({
                commitment: _commitment,
                timestamp: block.timestamp
            });
        }
        dataReferenceCount[dataId]++;
        emit DataListed(dataId);
        return dataId;
    }

    /**
     * @notice Owner cannot delist data directly, extra logic required.
     */
    function _delistDataInfo(bytes32 dataId) internal whenNotPaused {
        dataReferenceCount[dataId]--;
        if (dataReferenceCount[dataId] == 0) {
            delete dataInfoList[dataId];
        }
        emit DataDelisted(dataId);
    }

    // 提交 VDD 证明并触发 Oracle 检查
    function submitVDDProof(
        bytes calldata proof,
        bytes calldata publicValues,
        bytes calldata cOrigin,
        bytes memory cCipher // 加密后的密文，用于 Oracle 校验存储节点
    ) public onlyOwner {
        bytes32 bindHash = keccak256(
            abi.encode(cOrigin, dataKeyCommitment, cCipher)
        );
        // ======
        // 1. ZK Verification
        require(
            vddVerifier.verifyVDD(proof, publicValues, bindHash),
            "VDD verification failed"
        );

        vddVerified[cCipher] = true;

        _triggerOracle(cCipher);
        emit VDDProofSubmitted(cCipher);
    }

    function triggerOracle(bytes memory cCipher) public onlyOwner {
        _triggerOracle(cCipher);
    }

    function _triggerOracle(bytes memory cCipher) internal {
        require(vddVerified[cCipher], "VDD not verified");

        if (block.timestamp < lastOracleRequestAt[cCipher] + ORACLE_COOLDOWN) {
            emit OracleRequestSkipped(cCipher, "Cooldown active");
            return;
        }

        lastOracleRequestAt[cCipher] = block.timestamp;
        oracleWrapper.request(cCipher, address(this));
    }

    function onResponse(
        bytes memory cCipher,
        bytes memory response
    ) external virtual {
        require(msg.sender == address(oracleWrapper), "Only oracle proxy");

        // 1. 基础长度校验，防止 abi.decode 溢出或报错
        require(response.length == 64, "Invalid response length");

        (uint256 status, uint256 endTime) = abi.decode(
            response,
            (uint256, uint256)
        );

        // 2. 业务边界校验（注入防范）
        // 防止 Oracle 返回一个极大的时间戳导致系统逻辑溢出
        require(endTime < block.timestamp + 10 * 365 days, "EndTime too far");

        // 3. 状态校验
        if (status > 2) revert("Unknown status from oracle");

        delete lastOracleRequestAt[cCipher];

        if (status == 2) {
            // Ensured
            onSuccess(cCipher, endTime);
        }
        if (status == 1) {
            // Retriveable
            onSuccess(cCipher, block.timestamp + GRACE_PERIOD);
        }
        if (status == 0) {
            // Not retrievable
            onFail(cCipher);
        }
    }

    // Oracle 异步回调：验证成功
    function onSuccess(bytes memory cCipher, uint256 endTime) internal {
        require(msg.sender == address(oracleWrapper), "Only oracle proxy");
        if (!vddVerified[cCipher]) {
            return;
        }
        oracleSuccessUntil[cCipher] = endTime;
    }

    // Oracle 异步回调：验证失败
    function onFail(bytes memory cCipher) internal {
        require(msg.sender == address(oracleWrapper), "Only oracle proxy");
        if (!vddVerified[cCipher]) {
            return;
        }
        oracleSuccessUntil[cCipher] = 0;
    }
}

// src/interfaces/IExchangeHub.sol

interface IExchangeHub {
    function reportListEvent(bytes32 saleId, bytes memory dataCommitment, uint256 price, bytes32 version, string memory info) external;
    function reportUpdateEvent(bytes32 saleId, bytes memory dataCommitment, uint256 newPrice, bytes32 version, string memory info) external;
    function reportDelistEvent(bytes32 saleId) external;
    function reportPurchaseEvent(bytes32 saleId, bytes memory dataCommitment, address buyer, uint256 price, ExchangeInfo memory exchangeInfo) external;
    function reportSettleEvent(address buyer, bytes32 saleId, bytes memory dataCommitment) external;
    function reportRefundEvent(address buyer, bytes32 saleId, bytes memory dataCommitment, uint256 amount) external;
}

// src/ExchangeChannel.sol

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

struct ExchangeInfo {
    bytes32 saleDigest;
    uint256 price;
    uint256 initTime;
    uint256 deadline;
    bytes dataCommitment;
    Types.Hash vssKeyCommitment;
}

contract ExchangeChannelStorage is VDD, ReentrancyGuard {
    using Types for *;

    uint256 public constant LIVING_WINDOW = 7 days;

    IExchangeHub public hub;

    uint256 public nonce;

    bool public isInitialized;

    // sale_id => data_id
    mapping(bytes32 => bytes32) public saleVersions;
    mapping(bytes32 => bool) public pendingExchanges;
    mapping(address => uint256) public lockedBalances;

    constructor(
        Types.Pubkey memory _ownerPubKey,
        address _oracleWrapper,
        address _hub,
        address _owner,
        address vssVerifier,
        address vddVerifier
    ) VDD(_ownerPubKey, _oracleWrapper, _owner, vssVerifier, vddVerifier) {
        hub = IExchangeHub(_hub);
    }

    function initialize(
        Types.Pubkey memory _ownerPubKey,
        address _oracleWrapper,
        address _hub,
        address _owner,
        address vssVerifier,
        address vddVerifier
    ) external {
        require(!isInitialized, "Already initialized");
        init_VDD(
            _ownerPubKey,
            _oracleWrapper,
            _owner,
            vssVerifier,
            vddVerifier
        );
        hub = IExchangeHub(_hub);
    }
}

contract ExchangeChannelImplementation is ExchangeChannelStorage {
    using Types for *;

    constructor(
        Types.Pubkey memory _ownerPubKey,
        address _oracleWrapper,
        address _hub,
        address _owner,
        address vssVerifier,
        address vddVerifier
    )
        ExchangeChannelStorage(
            _ownerPubKey,
            _oracleWrapper,
            _hub,
            _owner,
            vssVerifier,
            vddVerifier
        )
    {}

    function getNextSaleId() public view returns (bytes32) {
        return keccak256(abi.encodePacked(address(this), block.chainid, nonce));
    }

    function listFile(
        Types.DataCommitment memory _commitment,
        uint256 price,
        string memory info
    ) public onlyOwner {
        bytes32 saleId = getNextSaleId();
        nonce = nonce + 1;

        // save data info
        bytes32 data_id = _listDataInfo(_commitment);

        // update data version
        saleVersions[saleId] = data_id;

        hub.reportListEvent(saleId, _commitment.data, price, data_id, info);
    }

    function updateFile(
        bytes32 saleId,
        Types.DataCommitment memory _commitment,
        uint256 _size,
        uint256 newPrice,
        string memory info
    ) public onlyOwner {
        bytes32 oldDataId = saleVersions[saleId];
        require(oldDataId != bytes32(0), "Sale does not exist");

        bytes32 newDataId = getDataId(_commitment.data);

        saleVersions[saleId] = newDataId;

        _delistDataInfo(oldDataId);
        _listDataInfo(_commitment);

        hub.reportUpdateEvent(
            saleId,
            _commitment.data,
            newPrice,
            saleVersions[saleId],
            info
        );
    }

    function delistFile(bytes32 saleId) public onlyOwner {
        bytes32 oldDataId = saleVersions[saleId];

        _delistDataInfo(oldDataId);

        delete saleVersions[saleId];

        hub.reportDelistEvent(saleId);
    }

    function getExchangeDigest(
        address buyer,
        ExchangeInfo memory info,
        bytes32 dataVersion
    ) public pure returns (bytes32) {
        return
            keccak256(
                abi.encodePacked(
                    buyer,
                    info.saleDigest,
                    dataVersion,
                    info.price,
                    info.initTime,
                    info.deadline,
                    info.dataCommitment,
                    info.vssKeyCommitment
                )
            );
    }

    // --- Actions ---

    function purchase(
        bytes32 saleId,
        bytes32 dataVersion,
        uint256 price,
        uint256 deadline,
        bytes calldata dataCommitment,
        Types.Hash vssKeyCommitment,
        Types.Cipher32 encryptedVssKey
    ) external payable {
        require(dataVersion == saleVersions[saleId], "Wrong data version");
        require(msg.value == price, "Exact price required");

        if (vssKeyCommitment.eq(bytes32(0).toHash())) {
            require(isRegistered[msg.sender], "Require vss key");
            vssKeyCommitment = audienceList[audienceIndex[msg.sender]]
                .vssKeyCommitment;
        }

        ExchangeInfo memory info = ExchangeInfo(
            saleId,
            price,
            block.timestamp,
            deadline,
            dataCommitment,
            vssKeyCommitment
        );
        bytes32 digest = getExchangeDigest(msg.sender, info, dataVersion);
        require(!pendingExchanges[digest], "Exchange already pending");
        pendingExchanges[digest] = true;
        lockedBalances[msg.sender] += msg.value;

        if (!isRegistered[msg.sender]) {
            _addAudience(msg.sender, vssKeyCommitment, encryptedVssKey);
        } else {
            require(
                audienceList[audienceIndex[msg.sender]].vssKeyCommitment.eq(
                    vssKeyCommitment
                ),
                "Inconsistent vss key"
            );
        }
        hub.reportPurchaseEvent(
            saleId,
            dataCommitment,
            msg.sender,
            price,
            info
        );
    }

    /**
     * @notice Seller fulfills requirements for a buyer.
     * @param vss vss proof is optinal, only required if isPrivy is false
     * @param vdd vdd proof is optional, only required if vddVerified is false
     */
    function fulfill(
        address buyer,
        ExchangeInfo calldata info,
        bytes32 dataVersion,
        VSSArgs calldata vss,
        VDDArgs calldata vdd
    ) external onlyOwner {
        require(block.timestamp <= info.deadline, "Not allow to fulfill");

        bytes32 digest = getExchangeDigest(buyer, info, dataVersion);
        require(pendingExchanges[digest], "No Exchange");

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
            oracleSuccessUntil[vdd.cCipher] <= info.initTime + LIVING_WINDOW
        ) {
            _triggerOracle(vdd.cCipher);
        }
    }

    /**
     * @notice Settlement: Can be called by anyone once requirements are met.
     */
    function settle(
        address buyer,
        ExchangeInfo calldata info,
        bytes32 dataVersion,
        bytes calldata cCipher
    ) external {
        bytes32 digest = getExchangeDigest(buyer, info, dataVersion);
        require(pendingExchanges[digest], "No Exchange");

        // Conditions for settlement:
        // 1. Buyer has keys (VSS Privy)
        require(isPrivy(buyer), "Buyer not privy");

        // 2. Data accessibility is confirmed
        require(
            oracleSuccessUntil[cCipher] > info.initTime + LIVING_WINDOW,
            "Oracle proof expired or missing"
        );

        delete pendingExchanges[digest];
        require(
            lockedBalances[buyer] >= info.price,
            "Insufficient locked balance"
        );
        lockedBalances[buyer] -= info.price;

        (bool success, ) = payable(owner).call{gas: 10_000, value: info.price}(
            ""
        );
        require(success, "Transfer failed");
        hub.reportSettleEvent(buyer, info.saleDigest, info.dataCommitment);
    }

    function refund(
        address buyer,
        ExchangeInfo calldata info,
        bytes32 dataVersion
    ) external nonReentrant {
        bytes32 digest = getExchangeDigest(buyer, info, dataVersion);
        require(pendingExchanges[digest], "No Exchange");
        require(block.timestamp > info.deadline, "Not expired");

        delete pendingExchanges[digest];
        require(
            lockedBalances[buyer] >= info.price,
            "Insufficient locked balance"
        );
        lockedBalances[buyer] -= info.price;

        (bool success, ) = payable(buyer).call{gas: 10_000, value: info.price}(
            ""
        );
        require(success, "Transfer failed");
        hub.reportRefundEvent(
            buyer,
            info.saleDigest,
            info.dataCommitment,
            info.price
        );
    }
}
