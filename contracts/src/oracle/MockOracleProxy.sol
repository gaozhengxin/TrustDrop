import "../interfaces/IOracleProxy.sol";

contract MockOracleProxy is IOracleProxy {
    event OracleRequested(address indexed client, bytes c_cipher);

    mapping(bytes => address) public requestToClient;

    function request(bytes memory c_cipher) external override {
        requestToClient[c_cipher] = msg.sender;
        emit OracleRequested(msg.sender, c_cipher);
    }
}
