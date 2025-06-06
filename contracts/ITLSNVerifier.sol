// contracts/ITLSNVerifier.sol
// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.20;

interface ITLSNVerifier {
    /// @notice Verify a TLS-Notary proof on-chain. Reverts if invalid.
    /// @param seal        zk-SNARK proof bytes (encoded by `encode_seal`).
    /// @param journalData ABI-encoded journal (bool,string,uint256,string).
    function verify(bytes calldata seal, bytes calldata journalData) external view;
}
