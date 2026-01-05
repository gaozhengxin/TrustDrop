interface IOracleClient {
    function onSuccess(bytes calldata cCipher) external;
    function onFail(bytes calldata cCipher) external;
}
