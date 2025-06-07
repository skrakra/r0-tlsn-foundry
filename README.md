# TLSN Verifier with Risc0 and Foundry

> Prove and verify [TLSNotary][tlsn-repo] (TLSN) proofs with the [RISC Zero zkVM][docs-zkvm] and validate results in your Ethereum smart contracts.

This repository extends the RISC Zero Foundry Template to integrate TLSN proof verification inside the RISC Zero zkVM:

1. Execute TLSN verification logic off-chain in the zkVM.  
2. Parse a score from the TLSNotary proof and assert it exceeds a specified threshold. 
3. Generate a Risc0 STARK and convert to SNARK using Bonsai or locally (only Linux supported) 
4. Submit and verify the SNARK proof on-chain via a Solidity verifier contract.

## Dependencies

Install the following prerequisites:

- **Rust** 
- **Foundry**  
- **RISC Zero toolchain**
- **Anvil**
- **lld**

## CLI

Update the submodule for risc0-ethereum 
```sh
git submodule update --init
```
From project root, make the RISC-V linker script executable:
```sh
chmod +x riscv32im-linker.sh
```
Export environment variables so Cargo/rustc inside the R0 container uses it:
```sh
export HOST_LINKER="$PWD/riscv32im-linker.sh"
```
Build the entire workspace in release mode using Docker and the custom linker:
```sh
RISC0_USE_DOCKER=1 \
  CARGO_TARGET_RISCV32IM_RISC0_ZKVM_ELF_LINKER="$HOST_LINKER" \
  cargo build --workspace --release
```
Optional: if you have bonsai api access otherwise you need to run this in a fully linux env as no other platform is supported yet: 
```sh
export BONSAI_API_KEY="YOUR_API_KEY"
export BONSAI_API_URL="https://rpc.bonsai.risc0.com"
```
Start Anvil in a seperate shell
```sh
anvil
```
Run the following command to submit a contract transaction (copy the required values from your Anvil instance):
```sh
cargo run --release --bin publisher -- \
  --chain-id 31337 \                # copy from anvil
  --eth-wallet-private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 \   # copy from anvil
  --rpc-url http://localhost:8545 \                     # copy from anvil
  --contract 0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512 \ # copy from anvil
  --proof-path apps/data/proof.json

```
If everything is successful, you should see:
```sh
✅ On-chain verification txn succeeded!
Transaction hash: 0x316e4b00f383151145be5f09c753372b30fb28229756699db7871f349032eb2e
```
then you can inspect the tx with: 
```sh
cast tx 0x316e4b00f383151145be5f09c753372b30fb28229756699db7871f349032eb2e --rpc-url http://localhost:8545
```
## Project Structure

Below are the primary files in the project directory

```text
.
├── Cargo.toml                      // Configuration for Cargo and Rust
├── foundry.toml                    // Configuration for Foundry
├── riscv32im-linker.sh             // Link wrapper for RISC-Zero guest via clang+lld
├── apps
│   ├── Cargo.toml
│   ├── data/proof.json             // tlsn proof
│   └── src
│       ├── lib.rs                  // Utility functions
│       └── bin                     
│           └── publisher.rs        // app to publish program results into the contract 
├── contracts
│   ├── TLSNVerifier.sol            // the tlsn verifier contract
│   ├── ImageID.sol                 // Generated contract with the image ID for your zkVM program
│   └── ...                         // other risc0 generated contracts
├── methods
│   ├── Cargo.toml
│   ├── .cargo/config.toml
│   ├── guest
│   │   ├── Cargo.toml
│   │   └── src
│   │       └── bin                 
│   │           └── tlsn_verifier.rs      // guest program for verifying a tlsn proof 
│   └── src
│       └── lib.rs                  // Compiled image IDs and tests for your guest programs
├── ring-patched                    // ring patch to work with risc-v support to force cargo to use it
└── tests
    ├── tlsnverifier.t.sol          // Tests for the tlsnverifier contract
    └── Elf.sol                     // Generated contract with paths the guest program ELF files.

```