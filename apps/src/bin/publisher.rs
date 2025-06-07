use alloy::{
    network::EthereumWallet,
    providers::ProviderBuilder,
    signers::local::PrivateKeySigner,
};
use alloy_primitives::Address;
use anyhow::{Context, Result};
use clap::Parser;

alloy::sol!(
    #[sol(rpc, all_derives)]
    interface IRiscZeroVerifier {
        function verify(bytes calldata seal, bytes32 imageId, bytes32 journalDigest) external view;
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

use methods::TLSN_VERIFIER_ELF;
use risc0_ethereum_contracts::encode_seal;
use risc0_zkvm::{default_prover, ExecutorEnv, ProverOpts, VerifierContext};
use std::{fs, path::PathBuf};
use url::Url;

/// Arguments of the publisher CLI
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Ethereum chain ID
    #[clap(long)]
    chain_id: u64,
    /// Ethereum private key
    #[clap(long, env)]
    eth_wallet_private_key: PrivateKeySigner,
    /// Ethereum Node endpoint
    #[clap(long)]
    rpc_url: Url,
    /// Deployed address of TLSNVerifier contract on-chain
    #[clap(long)]
    contract: Address,
    /// Path to the TLSN proof.json file to verify
    #[clap(short, long)]
    proof_path: PathBuf,
}

fn main() -> Result<()> {
    env_logger::init();
    let args = Args::parse();

    // Read the TLSN proof.json
    let proof_json = fs::read_to_string(&args.proof_path)
        .with_context(|| format!("Failed to read proof file {}", args.proof_path.display()))?;

    // Build the R0 Executor environment with the JSON string
    let exec_env = ExecutorEnv::builder().write(&proof_json)?.build()?;

    // Run the prover to produce a Receipt, using the TLSN_VERIFIER_ELF image:
    let receipt = default_prover()
        .prove_with_ctx(exec_env, &VerifierContext::default(), TLSN_VERIFIER_ELF, &ProverOpts::groth16())?
        .receipt;

    // Encode the "seal" for on-chain:
    let seal = encode_seal(&receipt)?;

    // Pull out the journal bytes (for TLSN: abi.encode(bool, string, uint256, string))
    let journal = receipt.journal.bytes.clone();

    // Build an Alloy provider + signer
    let wallet = EthereumWallet::from(args.eth_wallet_private_key);
    let provider = ProviderBuilder::new().wallet(wallet).connect_http(args.rpc_url);

    // Instantiate TLSNVerifier contract binding:
    let contract = TLSNVerifier::new(args.contract, provider);

    // Prepare the .verify(seal, journal) call
    let call_builder = contract.verify(seal.into(), journal.clone().into());

    // Send the transaction:
    let runtime = tokio::runtime::Runtime::new()?;
    let pending_tx = runtime.block_on(call_builder.send())?;
    let receipt = runtime.block_on(pending_tx.get_receipt())?;
    
    println!("✅ On-chain verification txn succeeded!");
    println!("Transaction hash: {:?}", receipt.transaction_hash);
    
    Ok(())
}