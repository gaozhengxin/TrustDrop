interface IOracleClient {
    function onResponse(bytes memory cCipher, bytes memory response) external;
}
