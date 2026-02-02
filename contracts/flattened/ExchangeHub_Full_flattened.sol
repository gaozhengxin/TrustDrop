// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.0 ^0.8.13 ^0.8.20;

// lib/openzeppelin-contracts/contracts/utils/Errors.sol

// OpenZeppelin Contracts (last updated v5.1.0) (utils/Errors.sol)

/**
 * @dev Collection of common custom errors used in multiple contracts
 *
 * IMPORTANT: Backwards compatibility is not guaranteed in future versions of the library.
 * It is recommended to avoid relying on the error API for critical functionality.
 *
 * _Available since v5.1._
 */
library Errors {
    /**
     * @dev The ETH balance of the account is not enough to perform the operation.
     */
    error InsufficientBalance(uint256 balance, uint256 needed);

    /**
     * @dev A call to an address target failed. The target may have reverted.
     */
    error FailedCall();

    /**
     * @dev The deployment failed.
     */
    error FailedDeployment();

    /**
     * @dev A necessary precompile is missing.
     */
    error MissingPrecompile(address);
}

// lib/openzeppelin-contracts/contracts/utils/LowLevelCall.sol

// OpenZeppelin Contracts (last updated v5.5.0) (utils/LowLevelCall.sol)

/**
 * @dev Library of low level call functions that implement different calling strategies to deal with the return data.
 *
 * WARNING: Using this library requires an advanced understanding of Solidity and how the EVM works. It is recommended
 * to use the {Address} library instead.
 */
library LowLevelCall {
    /// @dev Performs a Solidity function call using a low level `call` and ignoring the return data.
    function callNoReturn(address target, bytes memory data) internal returns (bool success) {
        return callNoReturn(target, 0, data);
    }

    /// @dev Same as {callNoReturn}, but allows to specify the value to be sent in the call.
    function callNoReturn(address target, uint256 value, bytes memory data) internal returns (bool success) {
        assembly ("memory-safe") {
            success := call(gas(), target, value, add(data, 0x20), mload(data), 0x00, 0x00)
        }
    }

    /// @dev Performs a Solidity function call using a low level `call` and returns the first 64 bytes of the result
    /// in the scratch space of memory. Useful for functions that return a tuple of single-word values.
    ///
    /// WARNING: Do not assume that the results are zero if `success` is false. Memory can be already allocated
    /// and this function doesn't zero it out.
    function callReturn64Bytes(
        address target,
        bytes memory data
    ) internal returns (bool success, bytes32 result1, bytes32 result2) {
        return callReturn64Bytes(target, 0, data);
    }

    /// @dev Same as {callReturnBytes32Pair}, but allows to specify the value to be sent in the call.
    function callReturn64Bytes(
        address target,
        uint256 value,
        bytes memory data
    ) internal returns (bool success, bytes32 result1, bytes32 result2) {
        assembly ("memory-safe") {
            success := call(gas(), target, value, add(data, 0x20), mload(data), 0x00, 0x40)
            result1 := mload(0x00)
            result2 := mload(0x20)
        }
    }

    /// @dev Performs a Solidity function call using a low level `staticcall` and ignoring the return data.
    function staticcallNoReturn(address target, bytes memory data) internal view returns (bool success) {
        assembly ("memory-safe") {
            success := staticcall(gas(), target, add(data, 0x20), mload(data), 0x00, 0x00)
        }
    }

    /// @dev Performs a Solidity function call using a low level `staticcall` and returns the first 64 bytes of the result
    /// in the scratch space of memory. Useful for functions that return a tuple of single-word values.
    ///
    /// WARNING: Do not assume that the results are zero if `success` is false. Memory can be already allocated
    /// and this function doesn't zero it out.
    function staticcallReturn64Bytes(
        address target,
        bytes memory data
    ) internal view returns (bool success, bytes32 result1, bytes32 result2) {
        assembly ("memory-safe") {
            success := staticcall(gas(), target, add(data, 0x20), mload(data), 0x00, 0x40)
            result1 := mload(0x00)
            result2 := mload(0x20)
        }
    }

    /// @dev Performs a Solidity function call using a low level `delegatecall` and ignoring the return data.
    function delegatecallNoReturn(address target, bytes memory data) internal returns (bool success) {
        assembly ("memory-safe") {
            success := delegatecall(gas(), target, add(data, 0x20), mload(data), 0x00, 0x00)
        }
    }

    /// @dev Performs a Solidity function call using a low level `delegatecall` and returns the first 64 bytes of the result
    /// in the scratch space of memory. Useful for functions that return a tuple of single-word values.
    ///
    /// WARNING: Do not assume that the results are zero if `success` is false. Memory can be already allocated
    /// and this function doesn't zero it out.
    function delegatecallReturn64Bytes(
        address target,
        bytes memory data
    ) internal returns (bool success, bytes32 result1, bytes32 result2) {
        assembly ("memory-safe") {
            success := delegatecall(gas(), target, add(data, 0x20), mload(data), 0x00, 0x40)
            result1 := mload(0x00)
            result2 := mload(0x20)
        }
    }

    /// @dev Returns the size of the return data buffer.
    function returnDataSize() internal pure returns (uint256 size) {
        assembly ("memory-safe") {
            size := returndatasize()
        }
    }

    /// @dev Returns a buffer containing the return data from the last call.
    function returnData() internal pure returns (bytes memory result) {
        assembly ("memory-safe") {
            result := mload(0x40)
            mstore(result, returndatasize())
            returndatacopy(add(result, 0x20), 0x00, returndatasize())
            mstore(0x40, add(result, add(0x20, returndatasize())))
        }
    }

    /// @dev Revert with the return data from the last call.
    function bubbleRevert() internal pure {
        assembly ("memory-safe") {
            let fmp := mload(0x40)
            returndatacopy(fmp, 0x00, returndatasize())
            revert(fmp, returndatasize())
        }
    }

    function bubbleRevert(bytes memory returndata) internal pure {
        assembly ("memory-safe") {
            revert(add(returndata, 0x20), mload(returndata))
        }
    }
}

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

    function init_owner(address _owner) internal {
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
    uint256 private constant _NOT_ENTERED = 0;
    uint256 private constant _ENTERED = 1;

    uint256 private _status;

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

// lib/openzeppelin-contracts/contracts/utils/Create2.sol

// OpenZeppelin Contracts (last updated v5.5.0) (utils/Create2.sol)

/**
 * @dev Helper to make usage of the `CREATE2` EVM opcode easier and safer.
 * `CREATE2` can be used to compute in advance the address where a smart
 * contract will be deployed, which allows for interesting new mechanisms known
 * as 'counterfactual interactions'.
 *
 * See the https://eips.ethereum.org/EIPS/eip-1014#motivation[EIP] for more
 * information.
 */
library Create2 {
    /**
     * @dev There's no code to deploy.
     */
    error Create2EmptyBytecode();

    /**
     * @dev Deploys a contract using `CREATE2`. The address where the contract
     * will be deployed can be known in advance via {computeAddress}.
     *
     * The bytecode for a contract can be obtained from Solidity with
     * `type(contractName).creationCode`.
     *
     * Requirements:
     *
     * - `bytecode` must not be empty.
     * - `salt` must have not been used for `bytecode` already.
     * - the factory must have a balance of at least `amount`.
     * - if `amount` is non-zero, `bytecode` must have a `payable` constructor.
     */
    function deploy(uint256 amount, bytes32 salt, bytes memory bytecode) internal returns (address addr) {
        if (address(this).balance < amount) {
            revert Errors.InsufficientBalance(address(this).balance, amount);
        }
        if (bytecode.length == 0) {
            revert Create2EmptyBytecode();
        }
        assembly ("memory-safe") {
            addr := create2(amount, add(bytecode, 0x20), mload(bytecode), salt)
        }
        if (addr == address(0)) {
            if (LowLevelCall.returnDataSize() == 0) {
                revert Errors.FailedDeployment();
            } else {
                LowLevelCall.bubbleRevert();
            }
        }
    }

    /**
     * @dev Returns the address where a contract will be stored if deployed via {deploy}. Any change in the
     * `bytecodeHash` or `salt` will result in a new destination address.
     */
    function computeAddress(bytes32 salt, bytes32 bytecodeHash) internal view returns (address) {
        return computeAddress(salt, bytecodeHash, address(this));
    }

    /**
     * @dev Returns the address where a contract will be stored if deployed via {deploy} from a contract located at
     * `deployer`. If `deployer` is this contract's address, returns the same value as {computeAddress}.
     */
    function computeAddress(bytes32 salt, bytes32 bytecodeHash, address deployer) internal pure returns (address addr) {
        assembly ("memory-safe") {
            let ptr := mload(0x40) // Get free memory pointer

            // |                     | ↓ ptr ...  ↓ ptr + 0x0B (start) ...  ↓ ptr + 0x20 ...  ↓ ptr + 0x40 ...   |
            // |---------------------|---------------------------------------------------------------------------|
            // | bytecodeHash        |                                                        CCCCCCCCCCCCC...CC |
            // | salt                |                                      BBBBBBBBBBBBB...BB                   |
            // | deployer            | 000000...0000AAAAAAAAAAAAAAAAAAA...AA                                     |
            // | 0xFF                |            FF                                                             |
            // |---------------------|---------------------------------------------------------------------------|
            // | memory              | 000000...00FFAAAAAAAAAAAAAAAAAAA...AABBBBBBBBBBBBB...BBCCCCCCCCCCCCC...CC |
            // | keccak(start, 0x55) |            ↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑ |

            mstore(add(ptr, 0x40), bytecodeHash)
            mstore(add(ptr, 0x20), salt)
            mstore(ptr, deployer) // Right-aligned with 12 preceding garbage bytes
            let start := add(ptr, 0x0b) // The hashed data starts at the final garbage byte which we will set to 0xff
            mstore8(start, 0xff)
            addr := and(keccak256(start, 0x55), 0xffffffffffffffffffffffffffffffffffffffff)
        }
    }
}

// lib/openzeppelin-contracts/contracts/proxy/Clones.sol

// OpenZeppelin Contracts (last updated v5.5.0) (proxy/Clones.sol)

/**
 * @dev https://eips.ethereum.org/EIPS/eip-1167[ERC-1167] is a standard for
 * deploying minimal proxy contracts, also known as "clones".
 *
 * > To simply and cheaply clone contract functionality in an immutable way, this standard specifies
 * > a minimal bytecode implementation that delegates all calls to a known, fixed address.
 *
 * The library includes functions to deploy a proxy using either `create` (traditional deployment) or `create2`
 * (salted deterministic deployment). It also includes functions to predict the addresses of clones deployed using the
 * deterministic method.
 */
library Clones {
    error CloneArgumentsTooLong();

    /**
     * @dev Deploys and returns the address of a clone that mimics the behavior of `implementation`.
     *
     * This function uses the create opcode, which should never revert.
     *
     * WARNING: This function does not check if `implementation` has code. A clone that points to an address
     * without code cannot be initialized. Initialization calls may appear to be successful when, in reality, they
     * have no effect and leave the clone uninitialized, allowing a third party to initialize it later.
     */
    function clone(address implementation) internal returns (address instance) {
        return clone(implementation, 0);
    }

    /**
     * @dev Same as {xref-Clones-clone-address-}[clone], but with a `value` parameter to send native currency
     * to the new contract.
     *
     * WARNING: This function does not check if `implementation` has code. A clone that points to an address
     * without code cannot be initialized. Initialization calls may appear to be successful when, in reality, they
     * have no effect and leave the clone uninitialized, allowing a third party to initialize it later.
     *
     * NOTE: Using a non-zero value at creation will require the contract using this function (e.g. a factory)
     * to always have enough balance for new deployments. Consider exposing this function under a payable method.
     */
    function clone(address implementation, uint256 value) internal returns (address instance) {
        if (address(this).balance < value) {
            revert Errors.InsufficientBalance(address(this).balance, value);
        }
        assembly ("memory-safe") {
            // Cleans the upper 96 bits of the `implementation` word, then packs the first 3 bytes
            // of the `implementation` address with the bytecode before the address.
            mstore(0x00, or(shr(232, shl(96, implementation)), 0x3d602d80600a3d3981f3363d3d373d3d3d363d73000000))
            // Packs the remaining 17 bytes of `implementation` with the bytecode after the address.
            mstore(0x20, or(shl(120, implementation), 0x5af43d82803e903d91602b57fd5bf3))
            instance := create(value, 0x09, 0x37)
        }
        if (instance == address(0)) {
            revert Errors.FailedDeployment();
        }
    }

    /**
     * @dev Deploys and returns the address of a clone that mimics the behavior of `implementation`.
     *
     * This function uses the create2 opcode and a `salt` to deterministically deploy
     * the clone. Using the same `implementation` and `salt` multiple times will revert, since
     * the clones cannot be deployed twice at the same address.
     *
     * WARNING: This function does not check if `implementation` has code. A clone that points to an address
     * without code cannot be initialized. Initialization calls may appear to be successful when, in reality, they
     * have no effect and leave the clone uninitialized, allowing a third party to initialize it later.
     */
    function cloneDeterministic(address implementation, bytes32 salt) internal returns (address instance) {
        return cloneDeterministic(implementation, salt, 0);
    }

    /**
     * @dev Same as {xref-Clones-cloneDeterministic-address-bytes32-}[cloneDeterministic], but with
     * a `value` parameter to send native currency to the new contract.
     *
     * WARNING: This function does not check if `implementation` has code. A clone that points to an address
     * without code cannot be initialized. Initialization calls may appear to be successful when, in reality, they
     * have no effect and leave the clone uninitialized, allowing a third party to initialize it later.
     *
     * NOTE: Using a non-zero value at creation will require the contract using this function (e.g. a factory)
     * to always have enough balance for new deployments. Consider exposing this function under a payable method.
     */
    function cloneDeterministic(
        address implementation,
        bytes32 salt,
        uint256 value
    ) internal returns (address instance) {
        if (address(this).balance < value) {
            revert Errors.InsufficientBalance(address(this).balance, value);
        }
        assembly ("memory-safe") {
            // Cleans the upper 96 bits of the `implementation` word, then packs the first 3 bytes
            // of the `implementation` address with the bytecode before the address.
            mstore(0x00, or(shr(232, shl(96, implementation)), 0x3d602d80600a3d3981f3363d3d373d3d3d363d73000000))
            // Packs the remaining 17 bytes of `implementation` with the bytecode after the address.
            mstore(0x20, or(shl(120, implementation), 0x5af43d82803e903d91602b57fd5bf3))
            instance := create2(value, 0x09, 0x37, salt)
        }
        if (instance == address(0)) {
            revert Errors.FailedDeployment();
        }
    }

    /**
     * @dev Computes the address of a clone deployed using {Clones-cloneDeterministic}.
     */
    function predictDeterministicAddress(
        address implementation,
        bytes32 salt,
        address deployer
    ) internal pure returns (address predicted) {
        assembly ("memory-safe") {
            let ptr := mload(0x40)
            mstore(add(ptr, 0x38), deployer)
            mstore(add(ptr, 0x24), 0x5af43d82803e903d91602b57fd5bf3ff)
            mstore(add(ptr, 0x14), implementation)
            mstore(ptr, 0x3d602d80600a3d3981f3363d3d373d3d3d363d73)
            mstore(add(ptr, 0x58), salt)
            mstore(add(ptr, 0x78), keccak256(add(ptr, 0x0c), 0x37))
            predicted := and(keccak256(add(ptr, 0x43), 0x55), 0xffffffffffffffffffffffffffffffffffffffff)
        }
    }

    /**
     * @dev Computes the address of a clone deployed using {Clones-cloneDeterministic}.
     */
    function predictDeterministicAddress(
        address implementation,
        bytes32 salt
    ) internal view returns (address predicted) {
        return predictDeterministicAddress(implementation, salt, address(this));
    }

    /**
     * @dev Deploys and returns the address of a clone that mimics the behavior of `implementation` with custom
     * immutable arguments. These are provided through `args` and cannot be changed after deployment. To
     * access the arguments within the implementation, use {fetchCloneArgs}.
     *
     * This function uses the create opcode, which should never revert.
     *
     * WARNING: This function does not check if `implementation` has code. A clone that points to an address
     * without code cannot be initialized. Initialization calls may appear to be successful when, in reality, they
     * have no effect and leave the clone uninitialized, allowing a third party to initialize it later.
     */
    function cloneWithImmutableArgs(address implementation, bytes memory args) internal returns (address instance) {
        return cloneWithImmutableArgs(implementation, args, 0);
    }

    /**
     * @dev Same as {xref-Clones-cloneWithImmutableArgs-address-bytes-}[cloneWithImmutableArgs], but with a `value`
     * parameter to send native currency to the new contract.
     *
     * WARNING: This function does not check if `implementation` has code. A clone that points to an address
     * without code cannot be initialized. Initialization calls may appear to be successful when, in reality, they
     * have no effect and leave the clone uninitialized, allowing a third party to initialize it later.
     *
     * NOTE: Using a non-zero value at creation will require the contract using this function (e.g. a factory)
     * to always have enough balance for new deployments. Consider exposing this function under a payable method.
     */
    function cloneWithImmutableArgs(
        address implementation,
        bytes memory args,
        uint256 value
    ) internal returns (address instance) {
        if (address(this).balance < value) {
            revert Errors.InsufficientBalance(address(this).balance, value);
        }
        bytes memory bytecode = _cloneCodeWithImmutableArgs(implementation, args);
        assembly ("memory-safe") {
            instance := create(value, add(bytecode, 0x20), mload(bytecode))
        }
        if (instance == address(0)) {
            revert Errors.FailedDeployment();
        }
    }

    /**
     * @dev Deploys and returns the address of a clone that mimics the behavior of `implementation` with custom
     * immutable arguments. These are provided through `args` and cannot be changed after deployment. To
     * access the arguments within the implementation, use {fetchCloneArgs}.
     *
     * This function uses the create2 opcode and a `salt` to deterministically deploy the clone. Using the same
     * `implementation`, `args` and `salt` multiple times will revert, since the clones cannot be deployed twice
     * at the same address.
     *
     * WARNING: This function does not check if `implementation` has code. A clone that points to an address
     * without code cannot be initialized. Initialization calls may appear to be successful when, in reality, they
     * have no effect and leave the clone uninitialized, allowing a third party to initialize it later.
     */
    function cloneDeterministicWithImmutableArgs(
        address implementation,
        bytes memory args,
        bytes32 salt
    ) internal returns (address instance) {
        return cloneDeterministicWithImmutableArgs(implementation, args, salt, 0);
    }

    /**
     * @dev Same as {xref-Clones-cloneDeterministicWithImmutableArgs-address-bytes-bytes32-}[cloneDeterministicWithImmutableArgs],
     * but with a `value` parameter to send native currency to the new contract.
     *
     * WARNING: This function does not check if `implementation` has code. A clone that points to an address
     * without code cannot be initialized. Initialization calls may appear to be successful when, in reality, they
     * have no effect and leave the clone uninitialized, allowing a third party to initialize it later.
     *
     * NOTE: Using a non-zero value at creation will require the contract using this function (e.g. a factory)
     * to always have enough balance for new deployments. Consider exposing this function under a payable method.
     */
    function cloneDeterministicWithImmutableArgs(
        address implementation,
        bytes memory args,
        bytes32 salt,
        uint256 value
    ) internal returns (address instance) {
        bytes memory bytecode = _cloneCodeWithImmutableArgs(implementation, args);
        return Create2.deploy(value, salt, bytecode);
    }

    /**
     * @dev Computes the address of a clone deployed using {Clones-cloneDeterministicWithImmutableArgs}.
     */
    function predictDeterministicAddressWithImmutableArgs(
        address implementation,
        bytes memory args,
        bytes32 salt,
        address deployer
    ) internal pure returns (address predicted) {
        bytes memory bytecode = _cloneCodeWithImmutableArgs(implementation, args);
        return Create2.computeAddress(salt, keccak256(bytecode), deployer);
    }

    /**
     * @dev Computes the address of a clone deployed using {Clones-cloneDeterministicWithImmutableArgs}.
     */
    function predictDeterministicAddressWithImmutableArgs(
        address implementation,
        bytes memory args,
        bytes32 salt
    ) internal view returns (address predicted) {
        return predictDeterministicAddressWithImmutableArgs(implementation, args, salt, address(this));
    }

    /**
     * @dev Get the immutable args attached to a clone.
     *
     * - If `instance` is a clone that was deployed using `clone` or `cloneDeterministic`, this
     *   function will return an empty array.
     * - If `instance` is a clone that was deployed using `cloneWithImmutableArgs` or
     *   `cloneDeterministicWithImmutableArgs`, this function will return the args array used at
     *   creation.
     * - If `instance` is NOT a clone deployed using this library, the behavior is undefined. This
     *   function should only be used to check addresses that are known to be clones.
     */
    function fetchCloneArgs(address instance) internal view returns (bytes memory) {
        bytes memory result = new bytes(instance.code.length - 0x2d); // revert if length is too short
        assembly ("memory-safe") {
            extcodecopy(instance, add(result, 0x20), 0x2d, mload(result))
        }
        return result;
    }

    /**
     * @dev Helper that prepares the initcode of the proxy with immutable args.
     *
     * An assembly variant of this function requires copying the `args` array, which can be efficiently done using
     * `mcopy`. Unfortunately, that opcode is not available before cancun. A pure solidity implementation using
     * abi.encodePacked is more expensive but also more portable and easier to review.
     *
     * NOTE: https://eips.ethereum.org/EIPS/eip-170[EIP-170] limits the length of the contract code to 24576 bytes.
     * With the proxy code taking 45 bytes, that limits the length of the immutable args to 24531 bytes.
     */
    function _cloneCodeWithImmutableArgs(
        address implementation,
        bytes memory args
    ) private pure returns (bytes memory) {
        if (args.length > 0x5fd3) revert CloneArgumentsTooLong();
        return
            abi.encodePacked(
                hex"61",
                uint16(args.length + 0x2d),
                hex"3d81600a3d39f3363d3d373d3d3d363d73",
                implementation,
                hex"5af43d82803e903d91602b57fd5bf3",
                args
            );
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
        address _owner,
        address _vssVerifier
    ) Ownable(_owner) {
        ownerPublicKey = _ownerPubKey;
        vssVerifier = IVSSVerifier(_vssVerifier);
    }

    function init_VSS(
        Types.Pubkey memory _ownerPubKey,
        address _owner,
        address _vssVerifier
    ) internal {
        init_owner(_owner);
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

        bytes32[] memory c_keys = new bytes32[](audiences.length);
        for (uint256 i = 0; i < audiences.length; i++) {
            require(isRegistered[audiences[i]], "Unregistered");
            c_keys[i] = audienceList[audienceIndex[audiences[i]]].vssKeyCommitment.unwrap();
        }

        bytes32 bindingHash = keccak256(
            abi.encode(dataKeyCommitment, c_keys, encryptedDataKeys)
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

// src/interfaces/IExchangeHub.sol

interface IExchangeHub {
    function reportListEvent(bytes32 saleId, bytes memory dataCommitment, uint256 price, bytes32 version, string memory info) external;
    function reportUpdateEvent(bytes32 saleId, bytes memory dataCommitment, uint256 newPrice, bytes32 version, string memory info) external;
    function reportDelistEvent(bytes32 saleId) external;
    function reportPurchaseEvent(bytes32 saleId, bytes memory dataCommitment, address buyer, uint256 price, ExchangeInfo memory exchangeInfo) external;
    function reportSettleEvent(address buyer, bytes32 saleId, bytes memory dataCommitment) external;
    function reportRefundEvent(address buyer, bytes32 saleId, bytes memory dataCommitment, uint256 amount) external;
}

// src/ExchangeHub.sol

contract ExchangeHub is IExchangeHub, Ownable {
    address public immutable implementation;

    IOracleProxy public immutable oracleWrapper;
    IVSSVerifier vssVerifier;
    IVDDVerifier vddVerifier;

    mapping(address => bool) public isRegisteredChannel;

    event ExchangeChannelCreated(
        address indexed owner,
        address indexed channel
    );
    event SaleListed(
        address indexed channel,
        bytes32 indexed saleId,
        bytes dataCommitment,
        uint256 price,
        bytes32 version,
        string info
    );
    event SaleUpdated(
        address indexed channel,
        bytes32 indexed saleId,
        bytes dataCommitment,
        uint256 newPrice,
        bytes32 version,
        string info
    );
    event SaleDelisted(address indexed channel, bytes32 indexed saleId);
    event PurchaseEvent(
        address indexed channel,
        bytes32 indexed saleId,
        bytes dataCommitment,
        address indexed buyer,
        uint256 price,
        ExchangeInfo exchangeInfo
    );
    event SettleEvent(
        address indexed channel,
        address indexed buyer,
        bytes32 indexed saleId,
        bytes dataCommitment
    );
    event RefundEvent(
        address indexed channel,
        address indexed buyer,
        bytes32 indexed saleId,
        bytes dataCommitment,
        uint256 amount
    );

    modifier onlyRegisteredChannel() {
        require(isRegisteredChannel[msg.sender], "Unauthorized channel");
        _;
    }

    constructor(
        address _oracleWrapper,
        address _vssVerifier,
        address _vddVerifier,
        address _implementation
    ) Ownable(msg.sender) {
        oracleWrapper = IOracleProxy(_oracleWrapper);
        vssVerifier = IVSSVerifier(_vssVerifier);
        vddVerifier = IVDDVerifier(_vddVerifier);
        implementation = _implementation;
    }

    function createExchangeChannel(
        Types.Pubkey memory ownerPubKey
    ) public returns (address) {
        address proxy = Clones.clone(implementation);
        ExchangeChannelStorage(proxy).initialize(
            ownerPubKey,
            address(oracleWrapper),
            address(this),
            msg.sender, // owner
            address(vssVerifier),
            address(vddVerifier)
        );

        isRegisteredChannel[proxy] = true;
        emit ExchangeChannelCreated(msg.sender, proxy);
        return proxy;
    }

    function reportListEvent(
        bytes32 saleId,
        bytes memory dataCommitment,
        uint256 price,
        bytes32 version,
        string memory info
    ) external override onlyRegisteredChannel {
        emit SaleListed(
            msg.sender,
            saleId,
            dataCommitment,
            price,
            version,
            info
        );
    }

    function reportUpdateEvent(
        bytes32 saleId,
        bytes memory dataCommitment,
        uint256 newPrice,
        bytes32 version,
        string memory info
    ) external override onlyRegisteredChannel {
        emit SaleUpdated(
            msg.sender,
            saleId,
            dataCommitment,
            newPrice,
            version,
            info
        );
    }

    function reportDelistEvent(
        bytes32 saleId
    ) external override onlyRegisteredChannel {
        emit SaleDelisted(msg.sender, saleId);
    }

    function reportPurchaseEvent(
        bytes32 saleId,
        bytes memory dataCommitment,
        address buyer,
        uint256 price,
        ExchangeInfo memory exchangeInfo
    ) external override onlyRegisteredChannel {
        emit PurchaseEvent(
            msg.sender,
            saleId,
            dataCommitment,
            buyer,
            price,
            exchangeInfo
        );
    }

    function reportSettleEvent(
        address buyer,
        bytes32 saleId,
        bytes memory dataCommitment
    ) external override onlyRegisteredChannel {
        emit SettleEvent(msg.sender, buyer, saleId, dataCommitment);
    }

    function reportRefundEvent(
        address buyer,
        bytes32 saleId,
        bytes memory dataCommitment,
        uint256 amount
    ) external override onlyRegisteredChannel {
        emit RefundEvent(msg.sender, buyer, saleId, dataCommitment, amount);
    }
}

