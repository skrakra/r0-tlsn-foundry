// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.20;

import {console2} from "forge-std/console2.sol";
import {Test} from "forge-std/Test.sol";
import {IRiscZeroVerifier, Receipt} from "risc0/IRiscZeroVerifier.sol";

contract DummyVerifier is IRiscZeroVerifier {
    function verify(bytes calldata, bytes32, bytes32) external pure override {}
    function verifyIntegrity(Receipt calldata) external pure override {}
}

// Your contract under test
contract TLSNVerifier {
    IRiscZeroVerifier public immutable verifier;
    bytes32 public constant imageId = 0xd553b34e4f354f823ba263b1c7d00d17127930c3cf3d5fae2deee0259ef78a62;

    constructor(IRiscZeroVerifier _verifier) {
        verifier = _verifier;
    }

    function verify(bytes calldata seal, bytes calldata journalData) public view {
        verifier.verify(seal, imageId, sha256(journalData));
        (bool isValid, string memory serverName, uint256 scoreWord, string memory errorMsg)
            = abi.decode(journalData, (bool, string, uint256, string));
        require(isValid, errorMsg);
        require(scoreWord > 5, "TLSN score <= 5");
    }
}

contract TLSNVerifierTest is Test {
    TLSNVerifier public tlsnVerifier;

    function setUp() public {
        IRiscZeroVerifier verifier = new DummyVerifier();
        tlsnVerifier = new TLSNVerifier(verifier);
    }

    function test_Deploy() public {
        assertTrue(address(tlsnVerifier) != address(0));
        assertTrue(address(tlsnVerifier.verifier()) != address(0));
        bytes32 expectedImageId = 0xd553b34e4f354f823ba263b1c7d00d17127930c3cf3d5fae2deee0259ef78a62;
        assertEq(tlsnVerifier.imageId(), expectedImageId);
    }

    function test_VerifyFailsWithLowScore() public {
        bytes memory journalData = abi.encode(true, "test.com", uint256(3), "");
        bytes memory mockSeal = "mock_seal_data";
        vm.expectRevert("TLSN score <= 5");
        tlsnVerifier.verify(mockSeal, journalData);
    }

    function test_VerifyFailsWithInvalidFlag() public {
        bytes memory journalData = abi.encode(false, "test.com", uint256(10), "Custom error message");
        bytes memory mockSeal = "mock_seal_data";
        vm.expectRevert("Custom error message");
        tlsnVerifier.verify(mockSeal, journalData);
    }

    function test_VerifySuccessWithValidData() public {
        bytes memory journalData = abi.encode(true, "test.com", uint256(10), "");
        bytes memory mockSeal = "mock_seal_data";
        // No revert expected
        tlsnVerifier.verify(mockSeal, journalData);
    }
}
