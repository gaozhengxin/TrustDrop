pragma solidity ^0.8.13;
interface IOracleProxy {
    function request(bytes memory c_cipher, address callback) external;
    function setWhitelist(address caller, bool allowed) external;
}
