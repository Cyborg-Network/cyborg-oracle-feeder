use once_cell::sync::{OnceCell};
use std::sync::Arc;
use subxt::{OnlineClient, PolkadotConfig};

pub static CLIENT: OnceCell<Arc<OnlineClient<PolkadotConfig>>> = OnceCell::new();

// Fails fast
pub async fn config(url: &str) {
    client(url).await;
}

async fn client(url: &str,) {
    let client = OnlineClient::<PolkadotConfig>::from_url(url).await
        .expect("Failed to initialize parachain client");
    CLIENT.set(Arc::new(client))
        .expect("CLIENT already set");
}