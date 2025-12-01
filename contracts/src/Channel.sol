import "./lib/Types.sol";
import "./lib/LibChannel.sol";

// TODO import IChannel from ./interfaces/...
// Channel implements IChannel
contract Channel {
    using Types for *;
    using LibChannel for *;

    event Followed(address indexed audience, address indexed channelAddress);
    event SessionKeyUpdated(
        address indexed channelAddress,
        uint64 indexed sessionVersion
    );
    event DataUpdated(address indexed channel, uint64 indexed dataVersion);
    event SenderTransferred(
        address indexed oldSender,
        address indexed newSender
    );
    event SenderKeyChanged(bytes indexed oldPubkey, bytes newPubkey);
    event SessionKeyCommitmentChanged(
        Types.Hash indexed oldCommitment,
        Types.Hash newCommitment
    );

    struct AudienceInfo {
        Types.Hash keyCommitment; // hash of a synmetric crypto key
        Types.Cipher32 audienceKeyEncrypted; // cipher of a synmetric crypto key which only sender can decrypt
    }

    struct VersionedCipher {
        Types.Cipher32 cipher;
        uint64 version;
    }

    address public controller;
    LibChannel.ChannelMetadata public metadata;
    address public immutable parentChannel;
    address public sender;
    bytes public senderPubkey;
    Types.Hash public sessionKeyCommitment;
    uint64 public sessionVersion = 0;

    modifier onlySender() {
        require(msg.sender == sender, "Channel: Unauthorized action");
        _;
    }

    modifier onlyController() {
        require(msg.sender == controller, "Channel: Unauthorized action");
        _;
    }

    mapping(address => bool) public isAudience;
    address[] private audienceList;

    mapping(address => AudienceInfo) public audienceData;

    mapping(address => VersionedCipher) public audienceSessionKeyCipher;

    address[] public childChannels;
    uint64 public version = 1;
    uint64 public parentVersion;

    constructor(
        address _controller,
        address _parentChannel,
        LibChannel.ChannelMetadata memory _metadata,
        bytes memory _senderPubkey,
        Types.Hash _sessionKeyCommitment
    ) payable {
        controller = _controller;
        parentChannel = _parentChannel;
        metadata = _metadata;
        sender = msg.sender;
        senderPubkey = _senderPubkey;
        sessionKeyCommitment = _sessionKeyCommitment;
    }

    function transferSender(address newSender) public onlySender {
        address oldSender = sender;
        sender = newSender;
        emit SenderTransferred(oldSender, newSender);
    }

    function changeSessionKeyCommitment(
        Types.Hash newCommitment
    ) public onlySender {
        Types.Hash oldCommitment = sessionKeyCommitment;
        sessionKeyCommitment = newCommitment;

        emit SessionKeyCommitmentChanged(oldCommitment, newCommitment);
    }

    function changeSenderPubkey(bytes memory newPubkey) public onlySender {
        bytes memory oldPubkey = senderPubkey;
        senderPubkey = newPubkey;

        emit SenderKeyChanged(oldPubkey, newPubkey);
    }

    function followChannel(
        address newAudience,
        Types.Hash audienceKeyCommitment,
        Types.Cipher32 audienceKeyEncrypted
    ) public onlyController {
        require(!isAudience[newAudience], "Channel: Already following");

        require(
            audienceList.length < metadata.maxAudience,
            "Channel: Max audience reached"
        );

        isAudience[newAudience] = true;
        audienceList.push(newAudience);

        audienceData[newAudience] = AudienceInfo({
            keyCommitment: audienceKeyCommitment,
            audienceKeyEncrypted: audienceKeyEncrypted
        });

        emit Followed(newAudience, address(this));
    }

    function submitEncryptedSessionKeys(
        address[] memory audienceAddrs,
        Types.Cipher32[] memory encryptedKeys
        // TODO require proof
    ) public {
        // TODO no auth required, proof is required
        require(
            audienceAddrs.length == encryptedKeys.length,
            "Channel: Key and address array length mismatch"
        );
        // TODO verify proof

        sessionVersion++;

        for (uint256 i = 0; i < audienceAddrs.length; i++) {
            address audience = audienceAddrs[i];
            Types.Cipher32 keyCipher = encryptedKeys[i];

            require(isAudience[audience], "Channel: Address is not a follower");

            audienceSessionKeyCipher[audience] = VersionedCipher({
                cipher: keyCipher,
                version: sessionVersion
            });
        }
        emit SessionKeyUpdated(address(this), sessionVersion);
    }

    function registerChild(address childAddress) public onlyController {
        require(
            childChannels.length < metadata.maxChild,
            "Channel: Max child channels reached"
        );

        childChannels.push(childAddress);
    }

    function updateData() public onlyController {
        version++;
        _notifyChildren();
        emit DataUpdated(address(this), version);
    }

    function updateParentVersion(uint64 newParentVersion) public {
        require(
            msg.sender == parentChannel,
            "Channel: Must be called by Parent"
        );
        parentVersion = newParentVersion;
    }

    function _notifyChildren() internal {
        require(
            msg.sender == sender || msg.sender == parentChannel,
            "Channel: Unauthorized notification"
        );

        version++;

        for (uint256 i = 0; i < childChannels.length; i++) {
            address childAddr = childChannels[i];

            // 使用 call 调用子合约上的 updateParentVersion(uint64) 函数
            bytes memory callData = abi.encodeWithSelector(
                bytes4(keccak256("updateParentVersion(uint64)")),
                version
            );

            (bool success, ) = childAddr.call(callData);
            require(success, "Channel: Child notification failed");
        }
    }
}
