// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import "forge-std/Test.sol";
import "../src/ExchangeHub.sol";
import "../src/oracle/OracleProxy.sol";
import "../src/interfaces/IVerifier.sol";

contract OracleReceiver {
    bytes public lastCipher;
    uint256 public lastStatus;
    uint256 public lastEndTime;
    bytes public lastError;

    function onResponse(bytes memory cCipher, bytes memory response) external {
        (uint256 status, uint256 endTime) = abi.decode(
            response,
            (uint256, uint256)
        );
        lastCipher = cCipher;
        lastStatus = status;
        lastEndTime = endTime;
    }

    function onOracleError(bytes memory cCipher, bytes memory err) external {
        lastCipher = cCipher;
        lastError = err;
    }

    function getLastCipher() external view returns (bytes memory) {
        return lastCipher;
    }

    function getLastError() external view returns (bytes memory) {
        return lastError;
    }
}

contract RevertingOracleReceiver {
    function onResponse(bytes memory, bytes memory) external pure {
        revert("receiver failed");
    }
}

contract OracleProxyHybridTest is Test {
    OracleProxy public proxy;
    OracleReceiver public receiver;

    address public owner = address(0xA11CE);
    address public controller = address(0xC011);
    address public requester = address(0xCAFE);
    address public signer = address(0x5151);
    address public forwarder = address(0xF00D);

    function setUp() public {
        vm.startPrank(owner);
        proxy = new OracleProxy(signer, forwarder);
        proxy.setController(controller);
        vm.stopPrank();

        vm.prank(controller);
        proxy.setWhitelist(requester, true);

        receiver = new OracleReceiver();
    }

    function _request(bytes memory cid) internal returns (bytes32 requestId) {
        return _requestFor(cid, address(receiver));
    }

    function _requestFor(
        bytes memory cid,
        address callback
    ) internal returns (bytes32 requestId) {
        uint256 requestNonce = proxy.nonce();
        requestId = keccak256(
            abi.encode(
                block.chainid,
                address(proxy),
                callback,
                cid,
                requestNonce
            )
        );
        vm.prank(requester);
        proxy.request(cid, callback);
    }

    function _report(
        bytes32 requestId,
        bytes memory cid,
        uint256 status,
        uint256 endTime,
        bytes memory err
    ) internal pure returns (bytes memory) {
        return abi.encode(requestId, cid, status, endTime, err);
    }

    function test_CentralizedReportUpdatesReceiver() public {
        bytes memory cid = hex"1234";
        bytes32 requestId = _request(cid);

        vm.prank(signer);
        proxy.submitCentralizedReport(_report(requestId, cid, 2, 123456, ""));

        assertEq(keccak256(receiver.getLastCipher()), keccak256(cid));
        assertEq(receiver.lastStatus(), 2);
        assertEq(receiver.lastEndTime(), 123456);
    }

    function test_Revert_CentralizedReportUnauthorizedSigner() public {
        bytes memory cid = hex"1234";
        bytes32 requestId = _request(cid);

        vm.prank(address(0xBAD));
        vm.expectRevert("Unauthorized signer");
        proxy.submitCentralizedReport(_report(requestId, cid, 2, 123456, ""));
    }

    function test_Revert_CentralizedReportReplay() public {
        bytes memory cid = hex"1234";
        bytes32 requestId = _request(cid);
        bytes memory report = _report(requestId, cid, 2, 123456, "");

        vm.prank(signer);
        proxy.submitCentralizedReport(report);

        vm.prank(signer);
        vm.expectRevert("Request fulfilled");
        proxy.submitCentralizedReport(report);
    }

    function test_Revert_CIDMismatch() public {
        bytes memory cid = hex"1234";
        bytes32 requestId = _request(cid);

        vm.prank(signer);
        vm.expectRevert("CID mismatch");
        proxy.submitCentralizedReport(
            _report(requestId, hex"9999", 2, 123456, "")
        );
    }

    function test_Revert_CallbackFailureDoesNotFulfillRequest() public {
        RevertingOracleReceiver badReceiver = new RevertingOracleReceiver();
        bytes memory cid = hex"1234";
        bytes32 requestId = _requestFor(cid, address(badReceiver));
        bytes memory report = _report(requestId, cid, 2, 123456, "");

        vm.prank(signer);
        vm.expectRevert("Callback failed");
        proxy.submitCentralizedReport(report);

        (, , , bool fulfilled) = proxy.requests(requestId);
        assertFalse(fulfilled);
    }

    function test_CREReportUsesForwarder() public {
        vm.prank(owner);
        proxy.setDefaultMode(OracleProxy.OracleMode.ChainlinkCRE);

        bytes memory cid = hex"abcd";
        bytes32 requestId = _request(cid);

        vm.prank(forwarder);
        proxy.onReport("", _report(requestId, cid, 1, 999, ""));

        assertEq(keccak256(receiver.getLastCipher()), keccak256(cid));
        assertEq(receiver.lastStatus(), 1);
        assertEq(receiver.lastEndTime(), 999);
    }

    function test_Revert_CREReportUnauthorizedForwarder() public {
        vm.prank(owner);
        proxy.setDefaultMode(OracleProxy.OracleMode.ChainlinkCRE);

        bytes memory cid = hex"abcd";
        bytes32 requestId = _request(cid);

        vm.prank(address(0xBAD));
        vm.expectRevert("Unauthorized forwarder");
        proxy.onReport("", _report(requestId, cid, 1, 999, ""));
    }
}

contract ExchangeHubConfigTest is Test {
    ExchangeHub public hub;
    OracleProxy public oracleA;
    OracleProxy public oracleB;
    ExchangeChannelImplementation public implementation;
    MockVerifier public verifierA;
    MockVerifier public verifierB;

    address public owner = address(0x1);

    function setUp() public {
        vm.startPrank(owner);
        oracleA = new OracleProxy(address(0xA), address(0xB));
        oracleB = new OracleProxy(address(0xC), address(0xD));
        verifierA = new MockVerifier();
        verifierB = new MockVerifier();
        implementation = new ExchangeChannelImplementation(
            Types.Pubkey(hex"00"),
            address(0),
            address(0),
            owner,
            address(0),
            address(0)
        );
        hub = new ExchangeHub(
            address(oracleA),
            address(verifierA),
            address(verifierA),
            address(implementation)
        );
        oracleA.setController(address(hub));
        oracleB.setController(address(hub));
        vm.stopPrank();
    }

    function test_OwnerCanUpdateProtocolComponentsForNewChannels() public {
        vm.startPrank(owner);
        hub.setOracleWrapper(address(oracleB));
        hub.setVSSVerifier(address(verifierB));
        hub.setVDDVerifier(address(verifierB));

        address channelAddress = hub.createExchangeChannel(Types.Pubkey(hex"01"));
        vm.stopPrank();

        ExchangeChannelImplementation channel = ExchangeChannelImplementation(
            channelAddress
        );
        assertEq(address(channel.oracleWrapper()), address(oracleB));
        assertEq(address(channel.vssVerifier()), address(verifierB));
        assertEq(address(channel.vddVerifier()), address(verifierB));
        assertTrue(oracleB.whiteList(channelAddress));
    }
}
