// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import "./VDD.sol";
import "./interfaces/ITradeHub.sol";

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
