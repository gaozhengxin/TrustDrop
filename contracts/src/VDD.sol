// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

// TODO VDD is VSS
// provable data delivery
abstract contract VDD {
    address public storageNetworkOracle;

    // TODO struct dataObject {publisher, commitment, timestamp}
    // TODO mapping(Types.Hash => dataObject)

    // TODO construtor

    // TODO
    function send(
        address channel
        // TODO other params
    ) public {
        // TODO call updateData
        // call oracle to check accessibility
    }

    // TODO select proper function name
    function oracleCallback(address channel) public {
        // TODO oracle worker call this function
        // check blob commitment with
        // TODO check oracle result
        // if success, call onSuccess
        // if fail, call onFail
    }

    function onSuccess() virtual internal;

    function onFail() virtual internal;
}
