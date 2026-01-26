pragma solidity ^0.8.0;

abstract contract Ownable {
    address public owner;
    address public pendingOwner;

    event OwnershipTransferStarted(
        address indexed previousOwner,
        address indexed newOwner
    );
    event OwnershipTransferred(
        address indexed previousOwner,
        address indexed newOwner
    );
    event OwnershipTransferCanceled(address indexed pendingOwner);

    constructor(address _owner) {
        owner = _owner;
    }

    function init_owner(address _owner) internal {
        owner = _owner;
    }

    modifier onlyOwner() {
        require(msg.sender == owner, "Not owner");
        _;
    }

    function transferOwner(address newOwner) public virtual onlyOwner {
        require(newOwner != address(0), "Invalid address");
        require(newOwner != owner, "Already owner");

        pendingOwner = newOwner;
        emit OwnershipTransferStarted(owner, newOwner);
    }

    function cancelTransfer() public virtual onlyOwner {
        require(pendingOwner != address(0), "No pending transfer");

        emit OwnershipTransferCanceled(pendingOwner);
        pendingOwner = address(0);
    }

    function claimOwnership() public virtual {
        require(msg.sender == pendingOwner, "Not the pending owner");

        emit OwnershipTransferred(owner, pendingOwner);
        owner = pendingOwner;
        pendingOwner = address(0); // 清空状态
    }
}
