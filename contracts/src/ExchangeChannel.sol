// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import "./VDD.sol";
import {ReentrancyGuard} from "./lib/ReentrancyGuard.sol";
import {Types} from "./lib/Types.sol";
import "./interfaces/IExchangeHub.sol";

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
    uint256 public constant MIN_PURCHASE_DEADLINE = 1 hours;
    uint256 public constant MAX_PURCHASE_DEADLINE = 30 days;

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
        isInitialized = true;
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
        isInitialized = true;
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
        bytes calldata encryptedVssKey
    ) external payable {
        require(dataVersion == saleVersions[saleId], "Wrong data version");
        require(
            getDataId(dataCommitment) == dataVersion,
            "Wrong data commitment"
        );
        require(
            deadline >= block.timestamp + MIN_PURCHASE_DEADLINE,
            "Deadline too soon"
        );
        require(
            deadline <= block.timestamp + MAX_PURCHASE_DEADLINE,
            "Deadline too far"
        );
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

        // 2. Cipher vdd is confirmed
        require(vddVerified[cCipher], "VDD not verified for this cipher");

        // 3. Data accessibility is confirmed
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
