interface IVerifier {
    function verifyVSS(
        bytes calldata proof,
        bytes calldata publicValues
    ) external returns (bool);
    function verifyVDD(
        bytes calldata proof,
        bytes calldata publicValues
    ) external returns (bool);
}
