use std::str::FromStr;
use subxt_signer::{sr25519::Keypair, SecretUri};
use crate::error::{Error, Result};

pub fn load_cyborg_test_key() -> Result<Keypair> {
    let mnemonic = std::env::var("CYBORG_TEST_KEY")
        .map_err(|e| Error::Custom(e.to_string()))?;
    let uri: SecretUri = SecretUri::from_str(&mnemonic)
        .map_err(|e| Error::Custom(e.to_string()))?;
    let keypair: Keypair = Keypair::from_uri(&uri)
        .map_err(|e| Error::Custom(e.to_string()))?;
    Ok(keypair)
}