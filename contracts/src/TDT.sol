// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

// trustless data trading
// TODO TDT is VDD
contract TDT {
    // struct Trade { // trade info }

    // TODO define data structure to retrieve and iterate all living trades

    // TODO constructor

    // TODO post advertisment
    // TODO select proper function name
    function post() public {
        // TODO save message struct in storage, which includes blob hash (Types.Hash)
        // and an advertising message
        // and price (amount and token) (optional)
        // TODO spawn a channel
    }

    // TODO select proper function name
    function requestPurchase() public {
        // TODO lock money, require amount>price
        // TODO save record
    }

    // TODO select proper function name
    function cancel() public {
        // TODO only when not accepted
        // TODO cancel purchase request
    }

    // TODO select proper function name
    function acceptPurchaseRequest() public {
        // TODO only sender
        // TODO grant ERC1155 access token
        // TODO follow channel with access token
        // TODO set a deadline, max deadline is 1 day
    }

    // implement VDD
    function onSuccess() internal {
        // TODO record all verified trades
        // TODO settle all verified trades
    }

    // implement VDD
    function onFail() internal {
        // TODO do nothing
    }

    function settle() public {
        // close a trade
        // called by EOA or by oracle (through onSuccess) 
        // TODO call VDD.send, in which delivers blob and verifies proof
        // TODO unlock money and transfer
    }

    function refund() public {
        // TODO only when trade deadline reached and not settled
        // TODO get money back
        // TODO check records
    }
}
