// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/// @title SamplingChallengeVRFMock
/// @notice Shared challenge registry for sampling-proof extensions.
/// @dev This is deliberately not a production VRF. It derives and fulfills a
/// seed synchronously from block values so the hackathon flow needs one seller
/// transaction and no coordinator. A miner/validator can influence these values.
contract SamplingChallengeVRFMock {
    struct Challenge {
        address requester;
        bytes32 seed;
        bytes32 requestId;
        uint64 requestCount;
        uint64 blockNumber;
    }

    mapping(address requester => mapping(bytes32 challengeKey => Challenge)) public latestChallenges;

    event SeedRequested(
        bytes32 indexed challengeKey, address indexed requester, bytes32 indexed requestId, uint64 requestCount
    );
    event SeedFulfilled(
        bytes32 indexed challengeKey,
        address indexed requester,
        bytes32 indexed requestId,
        bytes32 seed,
        uint64 requestCount
    );

    /// @notice Request and synchronously fulfill a new seed for a reusable key.
    /// Repeated requests are allowed; latestChallenges always exposes the newest.
    function requestSeed(bytes32 challengeKey) external returns (bytes32 requestId, bytes32 seed) {
        require(challengeKey != bytes32(0), "zero challenge key");
        uint64 requestCount = latestChallenges[msg.sender][challengeKey].requestCount + 1;
        requestId = keccak256(abi.encode(address(this), block.chainid, challengeKey, msg.sender, requestCount));
        seed = keccak256(
            abi.encode(
                bytes32("SAMPLING_VRF_MOCK_V1"),
                blockhash(block.number - 1),
                block.prevrandao,
                address(this),
                block.chainid,
                challengeKey,
                msg.sender,
                requestCount
            )
        );

        emit SeedRequested(challengeKey, msg.sender, requestId, requestCount);
        this.fulfillSeed(challengeKey, msg.sender, requestId, seed, requestCount);
    }

    /// @dev External self-call models the coordinator callback boundary.
    function fulfillSeed(bytes32 challengeKey, address requester, bytes32 requestId, bytes32 seed, uint64 requestCount)
        external
    {
        require(msg.sender == address(this), "only self callback");
        latestChallenges[requester][challengeKey] = Challenge({
            requester: requester,
            seed: seed,
            requestId: requestId,
            requestCount: requestCount,
            blockNumber: uint64(block.number)
        });
        emit SeedFulfilled(challengeKey, requester, requestId, seed, requestCount);
    }
}
