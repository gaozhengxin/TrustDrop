pragma solidity ^0.8.13;

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