// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "forge-std/Test.sol";
import "../GuardRailAnchor.sol";

contract GuardRailAnchorTest is Test {
    GuardRailAnchor public anchor;
    address public owner;
    address public authorizedUser;
    address public unauthorizedUser;

    bytes32 constant TEST_BATCH_ID = keccak256("test-batch-1");
    bytes32 constant TEST_MERKLE_ROOT = keccak256("test-merkle-root");
    uint32 constant TEST_EVENT_COUNT = 100;

    event BatchAnchored(
        bytes32 indexed batchId,
        bytes32 merkleRoot,
        uint32 eventCount,
        address indexed anchor,
        uint256 timestamp
    );

    event AnchorAuthorized(address indexed anchor);
    event AnchorRevoked(address indexed anchor);

    function setUp() public {
        owner = address(this);
        authorizedUser = makeAddr("authorized");
        unauthorizedUser = makeAddr("unauthorized");

        anchor = new GuardRailAnchor();

        // Authorize a test user
        anchor.authorizeAnchor(authorizedUser);
    }

    function test_Constructor() public {
        assertEq(anchor.owner(), owner);
        assertTrue(anchor.authorizedAnchors(owner));
        assertEq(anchor.anchorCooldown(), 60);
    }

    function test_StoreBatch_Success() public {
        vm.expectEmit(true, true, false, true);
        emit BatchAnchored(TEST_BATCH_ID, TEST_MERKLE_ROOT, TEST_EVENT_COUNT, owner, block.timestamp);

        anchor.storeBatch(TEST_MERKLE_ROOT, TEST_BATCH_ID, TEST_EVENT_COUNT);

        (bytes32 merkleRoot, uint32 eventCount, uint256 timestamp) = anchor.getBatch(TEST_BATCH_ID);
        assertEq(merkleRoot, TEST_MERKLE_ROOT);
        assertEq(eventCount, TEST_EVENT_COUNT);
        assertEq(timestamp, block.timestamp);
        assertEq(anchor.totalEventsAnchored(), TEST_EVENT_COUNT);
        assertEq(anchor.getBatchCount(), 1);
    }

    function test_StoreBatch_AuthorizedUser() public {
        vm.prank(authorizedUser);

        anchor.storeBatch(TEST_MERKLE_ROOT, TEST_BATCH_ID, TEST_EVENT_COUNT);

        assertTrue(anchor.verifyBatch(TEST_BATCH_ID, TEST_MERKLE_ROOT));
    }

    function test_StoreBatch_UnauthorizedUser() public {
        vm.prank(unauthorizedUser);

        vm.expectRevert(abi.encodeWithSelector(GuardRailAnchor.NotAuthorized.selector, unauthorizedUser));
        anchor.storeBatch(TEST_MERKLE_ROOT, TEST_BATCH_ID, TEST_EVENT_COUNT);
    }

    function test_StoreBatch_InvalidMerkleRoot() public {
        vm.expectRevert(GuardRailAnchor.InvalidMerkleRoot.selector);
        anchor.storeBatch(bytes32(0), TEST_BATCH_ID, TEST_EVENT_COUNT);
    }

    function test_StoreBatch_InvalidEventCount() public {
        vm.expectRevert(GuardRailAnchor.InvalidEventCount.selector);
        anchor.storeBatch(TEST_MERKLE_ROOT, TEST_BATCH_ID, 0);
    }

    function test_StoreBatch_BatchAlreadyExists() public {
        anchor.storeBatch(TEST_MERKLE_ROOT, TEST_BATCH_ID, TEST_EVENT_COUNT);

        vm.expectRevert(abi.encodeWithSelector(GuardRailAnchor.BatchAlreadyExists.selector, TEST_BATCH_ID));
        anchor.storeBatch(TEST_MERKLE_ROOT, TEST_BATCH_ID, TEST_EVENT_COUNT);
    }

    function test_StoreBatch_CooldownNotElapsed() public {
        anchor.storeBatch(TEST_MERKLE_ROOT, TEST_BATCH_ID, TEST_EVENT_COUNT);

        vm.expectRevert();
        anchor.storeBatch(keccak256("root2"), keccak256("batch2"), TEST_EVENT_COUNT);
    }

    function test_StoreBatch_WhenPaused() public {
        anchor.pause();

        vm.expectRevert("Pausable: paused");
        anchor.storeBatch(TEST_MERKLE_ROOT, TEST_BATCH_ID, TEST_EVENT_COUNT);
    }

    function test_GetBatch_NonExistent() public {
        vm.expectRevert(abi.encodeWithSelector(GuardRailAnchor.BatchNotFound.selector, TEST_BATCH_ID));
        anchor.getBatch(TEST_BATCH_ID);
    }

    function test_VerifyBatch_Success() public {
        anchor.storeBatch(TEST_MERKLE_ROOT, TEST_BATCH_ID, TEST_EVENT_COUNT);

        assertTrue(anchor.verifyBatch(TEST_BATCH_ID, TEST_MERKLE_ROOT));
        assertFalse(anchor.verifyBatch(TEST_BATCH_ID, keccak256("wrong-root")));
        assertFalse(anchor.verifyBatch(keccak256("wrong-batch"), TEST_MERKLE_ROOT));
    }

    function test_GetBatchIds_Pagination() public {
        // Store multiple batches
        bytes32 batchId1 = keccak256("batch1");
        bytes32 batchId2 = keccak256("batch2");
        bytes32 batchId3 = keccak256("batch3");

        anchor.storeBatch(keccak256("root1"), batchId1, 10);
        vm.warp(block.timestamp + 61); // Wait for cooldown
        anchor.storeBatch(keccak256("root2"), batchId2, 20);
        vm.warp(block.timestamp + 61);
        anchor.storeBatch(keccak256("root3"), batchId3, 30);

        // Test pagination
        bytes32[] memory ids = anchor.getBatchIds(0, 2);
        assertEq(ids.length, 2);
        assertEq(ids[0], batchId1);
        assertEq(ids[1], batchId2);

        ids = anchor.getBatchIds(2, 2);
        assertEq(ids.length, 1);
        assertEq(ids[0], batchId3);

        ids = anchor.getBatchIds(10, 2);
        assertEq(ids.length, 0);
    }

    function test_AuthorizeAnchor() public {
        vm.expectEmit(true, false, false, true);
        emit AnchorAuthorized(authorizedUser);

        anchor.authorizeAnchor(authorizedUser);
        assertTrue(anchor.authorizedAnchors(authorizedUser));
    }

    function test_AuthorizeAnchor_Unauthorized() public {
        vm.prank(unauthorizedUser);
        vm.expectRevert("Ownable: caller is not the owner");
        anchor.authorizeAnchor(makeAddr("new-user"));
    }

    function test_RevokeAnchor() public {
        anchor.authorizeAnchor(authorizedUser);

        vm.expectEmit(true, false, false, true);
        emit AnchorRevoked(authorizedUser);

        anchor.revokeAnchor(authorizedUser);
        assertFalse(anchor.authorizedAnchors(authorizedUser));
    }

    function test_SetCooldown() public {
        anchor.setCooldown(120);
        assertEq(anchor.anchorCooldown(), 120);
    }

    function test_Pause_Unpause() public {
        anchor.pause();
        assertTrue(anchor.paused());

        anchor.unpause();
        assertFalse(anchor.paused());
    }

    function testFuzz_StoreBatch(bytes32 merkleRoot, bytes32 batchId, uint32 eventCount) public {
        // Ensure valid inputs for fuzzing
        vm.assume(merkleRoot != bytes32(0));
        vm.assume(eventCount > 0);

        anchor.storeBatch(merkleRoot, batchId, eventCount);

        (bytes32 storedRoot, uint32 storedCount, uint256 storedTime) = anchor.getBatch(batchId);
        assertEq(storedRoot, merkleRoot);
        assertEq(storedCount, eventCount);
        assertEq(storedTime, block.timestamp);
    }

    function testFuzz_VerifyBatch(bytes32 merkleRoot, bytes32 batchId) public {
        vm.assume(merkleRoot != bytes32(0));

        anchor.storeBatch(merkleRoot, batchId, 1);
        assertTrue(anchor.verifyBatch(batchId, merkleRoot));
        assertFalse(anchor.verifyBatch(batchId, keccak256("wrong")));
    }
}