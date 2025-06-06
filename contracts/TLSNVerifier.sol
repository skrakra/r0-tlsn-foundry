// contracts/TLSNVerifier.sol
// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.20;

import "./IRiscZeroVerifier.sol";
import "./ImageID.sol";

contract TLSNVerifier {
    IRiscZeroVerifier public immutable verifier;
    bytes32 public constant imageId = ImageID.MAIN_ID;
    
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