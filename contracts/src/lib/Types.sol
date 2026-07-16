pragma solidity ^0.8.13;
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
