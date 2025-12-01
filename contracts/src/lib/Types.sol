library Types {
    enum HashType {
        SHA256, // 0
        BLAKE2B // 1
    }

    enum SynmetricKeyType {
        CHACHA8 // 0
    }

    enum PubkeyType {
        SECP256K1,
        ED25519
    }

    type Hash is bytes32;

    type SynmetricKey is bytes32;

    type Cipher32 is bytes32;

    function toHash(bytes32 b) internal pure returns (Hash) { return Hash.wrap(b); }
    function toCipher32(bytes32 b) internal pure returns (Cipher32) { return Cipher32.wrap(b); }
}