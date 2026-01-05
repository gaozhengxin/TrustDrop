import "./Types.sol";

library LibChannel {
    using Types for *;

    struct ChannelMetadata {
        Types.HashType hashType;
        Types.PubkeyType ownerKeyType;
        Types.SynmetricKeyType vssKeyType;
        Types.SynmetricKeyType dataKeyType;
    }
}