#![no_std]
#![no_main]

extern crate alloc;
use alloc::{
    format,
    str,
    string::{String, ToString},
};

use alloy_primitives::U256;
use alloy_sol_types::SolValue;
use bincode;
use hex;
use risc0_zkvm::guest::{env, entry};
use serde::{Deserialize, Serialize};
use tlsn_core::{
    presentation::{Presentation, PresentationOutput},
    signing::VerifyingKey as TlsnVerifyingKey,
    CryptoProvider,
};

entry!(main);

/// 33-byte compressed SEC-1 form of the Notary's public key
const EXPECTED_COMPRESSED_HEX: &str =
    "02d4cbba990b0c2eb1dd45b29c7d26075299f1ea39317f35140e6ef71e703beda7";

/// This is the Rust‐side representation (used when committing via Serde).
#[derive(Debug, Serialize, Deserialize)]
struct VerificationOutput {
    is_valid: bool,
    server_name: String,
    score: Option<u64>,
    error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct InputProofJson {
    #[serde(rename = "presentationJson")]
    presentation_json: InputPresentationData,
}

#[derive(Debug, Serialize, Deserialize)]
struct InputPresentationData {
    version: String,
    data: String,
}

fn main() {
    // 1. Read the outer JSON string from the host:
    let proof_json: String = env::read();

    // Initialize a "fallback" output for error‐cases:
    let mut output = VerificationOutput {
        is_valid: false,
        server_name: String::new(),
        score: None,
        error: None,
    };

    // 2. Parse the outer JSON:
    let input: InputProofJson = match serde_json::from_str(&proof_json) {
        Ok(v) => v,
        Err(e) => {
            output.error = Some(format!("Failed to parse outer JSON: {}", e));
            // Commit immediately with Serde—this branch will *not* be read on-chain.
            env::commit(&output);
            return;
        }
    };

    // 3. Hex‐decode the "data" field:
    let proof_bytes = match hex::decode(&input.presentation_json.data) {
        Ok(b) => b,
        Err(e) => {
            output.error = Some(format!("Failed to hex-decode data: {}", e));
            env::commit(&output);
            return;
        }
    };

    // 4. Bincode‐deserialize into a `Presentation`:
    let tlsn_presentation: Presentation = match bincode::deserialize(&proof_bytes) {
        Ok(p) => p,
        Err(e) => {
            output.error = Some(format!("Bincode deserialize failed: {}", e));
            env::commit(&output);
            return;
        }
    };

    // 5. Check that the embedded verifying key matches our expected constant:
    let embedded_vk: &TlsnVerifyingKey = tlsn_presentation.verifying_key();
    let embedded_hex = hex::encode(&embedded_vk.data);
    if embedded_hex != EXPECTED_COMPRESSED_HEX {
        output.error = Some(format!(
            "Key mismatch:\n  embedded = {}\n  expected = {}",
            embedded_hex, EXPECTED_COMPRESSED_HEX,
        ));
        env::commit(&output);
        return;
    }

    // 6. Run the actual TLSN "verify" logic:
    let provider = CryptoProvider::default();
    let pres_out: PresentationOutput = match tlsn_presentation.verify(&provider) {
        Ok(o) => o,
        Err(e) => {
            output.error = Some(format!("Presentation.verify() failed: {:?}", e));
            env::commit(&output);
            return;
        }
    };

    // 7. Populate `output` with data from `pres_out`:
    if let Some(sn) = pres_out.server_name {
        output.server_name = sn.to_string();
    }
    output.is_valid = true;

    // Extract "score" (if present) from the transcript:
    if let Some(transcript) = pres_out.transcript {
        if let Ok(s) = str::from_utf8(transcript.received_unsafe()) {
            if let Some(val) = s.split("score=").nth(1) {
                output.score = val
                    .split(&['&', '"'][..])
                    .next()
                    .and_then(|num| num.parse().ok());
            }
        }
    }

    // 8. Enforce a minimum‐score policy of > 5:
    match output.score {
        Some(score_val) if score_val > 5 => {
            // OK: above threshold
        }
        Some(score_val) => {
            output.error = Some(format!("Score {} is below the required threshold of 5", score_val));
            output.is_valid = false;
            env::commit(&output);
            return;
        }
        None => {
            output.error = Some("Score missing or could not be parsed".to_string());
            output.is_valid = false;
            env::commit(&output);
            return;
        }
    }

    // 9. ABI-encode for on-chain consumption
    // manually encode the struct fields as a tuple
    // (bool, string, uint256, string)
    let sol_score: U256 = match output.score {
        Some(val) if val > 0 => U256::from(val),
        _ => U256::from(0u8),
    };
    let sol_error_str: String = output.error.clone().unwrap_or_else(|| "".to_string());

    // Manual ABI encoding as tuple: (bool, string, uint256, string)
    let encoded_journal = (
        output.is_valid,
        output.server_name.clone(),
        sol_score,
        sol_error_str,
    ).abi_encode();

    // Commit the ABI-encoded journal bytes (so the on-chain verifier can decode them).
    env::commit_slice(encoded_journal.as_slice());
}