use crate::error::Result;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use subxt::{blocks::Block, OnlineClient, PolkadotConfig};
use tokio::sync::Mutex;

const LAST_BLOCK_FILE: &str = "last_block.txt";

pub struct BlockTracker {
    client: Arc<OnlineClient<PolkadotConfig>>,
    data_dir: PathBuf,
    last_processed_block: Mutex<Option<u32>>,
}

impl BlockTracker {
    pub fn new(client: Arc<OnlineClient<PolkadotConfig>>, data_dir: PathBuf) -> Result<Self> {
        // Create data directory if it doesn't exist
        if !data_dir.exists() {
            fs::create_dir_all(&data_dir)?;
        }

        let last_block_path = data_dir.join(LAST_BLOCK_FILE);
        let last_processed_block = if last_block_path.exists() {
            let block_num = fs::read_to_string(&last_block_path)?
                .trim()
                .parse()
                .unwrap_or(0);
            Some(block_num)
        } else {
            None // First run
        };

        Ok(Self {
            client,
            data_dir,
            last_processed_block: Mutex::new(last_processed_block),
        })
    }

    pub async fn needs_sync(&self) -> Result<bool> {
        let last_block = *self.last_processed_block.lock().await;
        if let Some(last_block_num) = last_block {
            let current_block = self.client.blocks().at_latest().await?.number();
            Ok(current_block > last_block_num)
        } else {
            Ok(false) // First run, no sync needed
        }
    }

    pub async fn update_last_block(&self, block_num: u32) -> Result<()> {
        let mut last_block = self.last_processed_block.lock().await;
        *last_block = Some(block_num);
        fs::write(self.data_dir.join(LAST_BLOCK_FILE), block_num.to_string())?;
        Ok(())
    }

    pub async fn get_missed_blocks(
        &self,
    ) -> Result<Vec<Block<PolkadotConfig, OnlineClient<PolkadotConfig>>>> {
        let last_block_num = self.last_processed_block.lock().await.unwrap_or(0);
        let current_block = self.client.blocks().at_latest().await?;
        let mut current_block_num = current_block.number();

        let mut missed_blocks = Vec::new();

        // Walk backwards from current block to last processed block
        while current_block_num > last_block_num {
            let block_hash = current_block.hash();
            let block = self.client.blocks().at(block_hash).await?;
            missed_blocks.push(block);
            current_block_num -= 1;
        }

        // Reverse to process in chronological order
        missed_blocks.reverse();
        Ok(missed_blocks)
    }
}
