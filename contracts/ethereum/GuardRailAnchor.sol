// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/access/Ownable.sol";
import "@openzeppelin/contracts/utils/Pausable.sol";

/**
 * @title GuardRailAnchor
 * @dev Stores Merkle roots of GuardRail audit event batches on-chain
 * @notice This contract provides immutable, verifiable proof of audit trail integrity
 */
contract GuardRailAnchor is Ownable, Pausable {
    // ============ Structs ============
    
    struct Batch {
        bytes32 merkleRoot;
        uint32 eventCount;
        uint256 timestamp;
        bool exists;
    }
    
    // ============ State Variables ============
    
    /// @notice Mapping from batch ID to batch data
    mapping(bytes32 => Batch) public batches;
    
    /// @notice Array of all batch IDs for enumeration
    bytes32[] public batchIds;
    
    /// @notice Authorized anchoring addresses
    mapping(address => bool) public authorizedAnchors;
    
    /// @notice Total events anchored across all batches
    uint256 public totalEventsAnchored;
    
    /// @notice Minimum time between anchors from same sender (anti-spam)
    uint256 public anchorCooldown = 60; // 1 minute default
    
    /// @notice Last anchor timestamp per address
    mapping(address => uint256) public lastAnchorTime;
    
    // ============ Events ============
    
    event BatchAnchored(
        bytes32 indexed batchId,
        bytes32 merkleRoot,
        uint32 eventCount,
        address indexed anchor,
        uint256 timestamp
    );
    
    event AnchorAuthorized(address indexed anchor);
    event AnchorRevoked(address indexed anchor);
    event CooldownUpdated(uint256 oldCooldown, uint256 newCooldown);
    
    // ============ Errors ============
    
    error BatchAlreadyExists(bytes32 batchId);
    error BatchNotFound(bytes32 batchId);
    error NotAuthorized(address sender);
    error CooldownNotElapsed(uint256 timeRemaining);
    error InvalidMerkleRoot();
    error InvalidEventCount();
    
    // ============ Modifiers ============
    
    modifier onlyAuthorized() {
        if (!authorizedAnchors[msg.sender] && msg.sender != owner()) {
            revert NotAuthorized(msg.sender);
        }
        _;
    }
    
    modifier cooldownElapsed() {
        uint256 elapsed = block.timestamp - lastAnchorTime[msg.sender];
        if (elapsed < anchorCooldown) {
            revert CooldownNotElapsed(anchorCooldown - elapsed);
        }
        _;
    }
    
    // ============ Constructor ============
    
    constructor() Ownable(msg.sender) {
        // Owner is automatically authorized
        authorizedAnchors[msg.sender] = true;
    }
    
    // ============ External Functions ============
    
    /**
     * @notice Store a new batch anchor
     * @param merkleRoot The Merkle root of the event batch
     * @param batchId Unique identifier for the batch
     * @param eventCount Number of events in the batch
     */
    function storeBatch(
        bytes32 merkleRoot,
        bytes32 batchId,
        uint32 eventCount
    ) external onlyAuthorized cooldownElapsed whenNotPaused {
        if (merkleRoot == bytes32(0)) revert InvalidMerkleRoot();
        if (eventCount == 0) revert InvalidEventCount();
        if (batches[batchId].exists) revert BatchAlreadyExists(batchId);
        
        batches[batchId] = Batch({
            merkleRoot: merkleRoot,
            eventCount: eventCount,
            timestamp: block.timestamp,
            exists: true
        });
        
        batchIds.push(batchId);
        totalEventsAnchored += eventCount;
        lastAnchorTime[msg.sender] = block.timestamp;
        
        emit BatchAnchored(batchId, merkleRoot, eventCount, msg.sender, block.timestamp);
    }
    
    /**
     * @notice Get batch details
     * @param batchId The batch ID to query
     * @return merkleRoot The Merkle root
     * @return eventCount Number of events
     * @return timestamp When the batch was anchored
     */
    function getBatch(bytes32 batchId) 
        external 
        view 
        returns (bytes32 merkleRoot, uint32 eventCount, uint256 timestamp) 
    {
        Batch storage batch = batches[batchId];
        if (!batch.exists) revert BatchNotFound(batchId);
        
        return (batch.merkleRoot, batch.eventCount, batch.timestamp);
    }
    
    /**
     * @notice Verify that a Merkle root matches the stored value
     * @param batchId The batch ID to verify
     * @param merkleRoot The Merkle root to check
     * @return valid True if the Merkle root matches
     */
    function verifyBatch(bytes32 batchId, bytes32 merkleRoot) 
        external 
        view 
        returns (bool valid) 
    {
        Batch storage batch = batches[batchId];
        if (!batch.exists) return false;
        
        return batch.merkleRoot == merkleRoot;
    }
    
    /**
     * @notice Get total number of batches
     * @return count Number of batches anchored
     */
    function getBatchCount() external view returns (uint256 count) {
        return batchIds.length;
    }
    
    /**
     * @notice Get batch IDs with pagination
     * @param offset Starting index
     * @param limit Maximum number to return
     * @return ids Array of batch IDs
     */
    function getBatchIds(uint256 offset, uint256 limit) 
        external 
        view 
        returns (bytes32[] memory ids) 
    {
        uint256 total = batchIds.length;
        if (offset >= total) {
            return new bytes32[](0);
        }
        
        uint256 end = offset + limit;
        if (end > total) {
            end = total;
        }
        
        ids = new bytes32[](end - offset);
        for (uint256 i = offset; i < end; i++) {
            ids[i - offset] = batchIds[i];
        }
        
        return ids;
    }
    
    // ============ Admin Functions ============
    
    /**
     * @notice Authorize an address to anchor batches
     * @param anchor Address to authorize
     */
    function authorizeAnchor(address anchor) external onlyOwner {
        authorizedAnchors[anchor] = true;
        emit AnchorAuthorized(anchor);
    }
    
    /**
     * @notice Revoke anchor authorization
     * @param anchor Address to revoke
     */
    function revokeAnchor(address anchor) external onlyOwner {
        authorizedAnchors[anchor] = false;
        emit AnchorRevoked(anchor);
    }
    
    /**
     * @notice Update the anchor cooldown period
     * @param newCooldown New cooldown in seconds
     */
    function setCooldown(uint256 newCooldown) external onlyOwner {
        emit CooldownUpdated(anchorCooldown, newCooldown);
        anchorCooldown = newCooldown;
    }
    
    /**
     * @notice Pause the contract
     */
    function pause() external onlyOwner {
        _pause();
    }
    
    /**
     * @notice Unpause the contract
     */
    function unpause() external onlyOwner {
        _unpause();
    }
}
