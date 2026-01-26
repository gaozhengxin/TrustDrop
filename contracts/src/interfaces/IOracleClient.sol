pragma solidity ^0.8.13;
interface IOracleClient {
    function onResponse(bytes memory cCipher, bytes memory response) external;
}
