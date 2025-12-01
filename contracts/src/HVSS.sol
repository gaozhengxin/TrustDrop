// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

// TODO import standard ERC1155 contract
// hierachical verifiable secret sharing
// TODO inherits ERC1155, HVSS is ERC1155 access token
contract HVSS {
    constructor() {
        // TODO initiate ERC1155
    }

    // TODO modify function name
    function spawn() public {
        // TODO create channel, set controller to address(this)
        // TODO initiate ERC1155 collection
        // Token ID == channel address (convert into token id)
        // ERC1155 total supply matches maxAudience
        // TODO revoke registerChild on parent channel if needed
    }

    // issue ERC1155 access token
    // TODO Only channel sender
    function issue(address channel) public {
        // TODO Mint ERC1155 token
    }

    // TODO modify function name and params
    function follow() public {
        // TODO verify ERC1155
        // TODO call channel.follow, set msg.sender as audience
    }

    // TODO return some values
    function channels() public view {
        // TODO return msg sender's token ids
        // TODO return msg sender's all channel session version
        // TODO return msg sender's all audienceSessionKeyCipher, which includes encrypted key and version
    }
}
