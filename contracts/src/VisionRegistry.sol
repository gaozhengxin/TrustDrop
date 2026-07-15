// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import {Ownable} from "./lib/Ownable.sol";

contract VisionRegistry is Ownable {
    string public visionCid;

    event VisionCidUpdated(string oldCid, string newCid);

    constructor(address admin, string memory initialVisionCid) Ownable(admin) {
        visionCid = initialVisionCid;
        emit VisionCidUpdated("", initialVisionCid);
    }

    function setVisionCid(string calldata newVisionCid) external onlyOwner {
        string memory oldCid = visionCid;
        visionCid = newVisionCid;
        emit VisionCidUpdated(oldCid, newVisionCid);
    }
}
