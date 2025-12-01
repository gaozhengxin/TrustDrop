import "./Types.sol";

library LibChannel {
    using Types for *;

    // 保持 HVSSMetadata 结构体不变
    struct ChannelMetadata {
        Types.HashType hashType;
        Types.PubkeyType senderKeyType;
        Types.SynmetricKeyType audienceKeyType;
        Types.SynmetricKeyType sessionKeyType;
        uint8 maxAudience;
        uint8 maxChild;
    }
}