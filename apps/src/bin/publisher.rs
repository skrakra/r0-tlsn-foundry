// publisher/src/main.rs
use alloy::{
    network::EthereumWallet,
    providers::ProviderBuilder,
    signers::local::PrivateKeySigner,
    sol_types::SolValue,
};
use alloy_primitives::Address;
use anyhow::{Context, Result};
use clap::Parser;

alloy::sol!(
    #[sol(rpc, all_derives)]
    interface IRiscZeroVerifier {
        function verify(bytes calldata seal, bytes32 imageId, bytes32 journalDigest) external view;
        // Removed verifyIntegrity to avoid Receipt type issues
    }

    #[sol(rpc, all_derives)]
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
);

use methods::MAIN_ELF;
use risc0_ethereum_contracts::encode_seal;
use risc0_zkvm::{default_prover, ExecutorEnv, ProverOpts, VerifierContext};
use std::{fs, path::PathBuf};
use url::Url;

/// Arguments of the publisher CLI.
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Ethereum chain ID
    #[clap(long)]
    chain_id: u64,
    /// Ethereum private key (Env var or passed directly).
    #[clap(long, env)]
    eth_wallet_private_key: PrivateKeySigner,
    /// Ethereum Node endpoint.
    #[clap(long)]
    rpc_url: Url,
    /// Deployed address of TLSNVerifier contract on-chain.
    #[clap(long)]
    contract: Address,
    /// Path to the TLSN proof JSON file to verify.
    #[clap(short, long)]
    proof_path: PathBuf,
}

fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse();

    // 1) Read the TLSN proof JSON from disk
    let proof_json = fs::read_to_string(&args.proof_path)
        .with_context(|| format!("Failed to read proof file {}", args.proof_path.display()))?;

    // 2) ABI-encode the JSON string so the guest can env::read() it.
    let input_bytes = proof_json.abi_encode();

    // 3) Build the RISC0 Executor environment with that input.
    let exec_env = ExecutorEnv::builder().write_slice(&input_bytes).build()?;

    // 4) Run the prover to produce a Receipt, using the MAIN_ELF image:
    let receipt = default_prover()
        .prove_with_ctx(exec_env, &VerifierContext::default(), MAIN_ELF, &ProverOpts::groth16())?
        .receipt;

    // 5) Encode the "seal" for on-chain:
    let seal = encode_seal(&receipt)?;

    // 6) Pull out the journal bytes (for TLSN: abi.encode(bool, string, uint256, string))
    let journal = receipt.journal.bytes.clone();

    // 7) Build an Alloy provider + signer
    let wallet = EthereumWallet::from(args.eth_wallet_private_key);
    let provider = ProviderBuilder::new().wallet(wallet).connect_http(args.rpc_url);

    // 8) Instantiate your TLSNVerifier contract binding:
    let contract = TLSNVerifier::new(args.contract, provider);

    // 9) Prepare the .verify(seal, journal) call.
    // Fixed parameter order: seal first, then journal
    let call_builder = contract.verify(seal.into(), journal.clone().into());

    // 10) Finally, send the transaction:
    let runtime = tokio::runtime::Runtime::new()?;
    let pending_tx = runtime.block_on(call_builder.send())?;
    let receipt = runtime.block_on(pending_tx.get_receipt())?; // Fixed syntax
    
    println!("✅ On-chain verification txn succeeded!");
    println!("Transaction hash: {:?}", receipt.transaction_hash);
    
    Ok(())
}