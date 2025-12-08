use crate::error::{Error, Result};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
#[cfg(feature = "integration")]
use subxt::{blocks::Block, OnlineClient, PolkadotConfig};
#[cfg(feature = "integration")]
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
            fs::create_dir_all(&data_dir).map_err(|e| Error::custom(e.to_string()))?;
        }

        let last_block_path = data_dir.join(LAST_BLOCK_FILE);
        let last_processed_block = if last_block_path.exists() {
            let block_num = fs::read_to_string(&last_block_path)
                .map_err(|e| Error::custom(e.to_string()))?
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

        // First startup - no sync needed, just update to current block
        if last_block.is_none() {
            let current_block = self
                .client
                .blocks()
                .at_latest()
                .await
                .map_err(|e| Error::custom(e.to_string()))?
                .number();
            self.update_last_block(current_block).await?;
            return Ok(false);
        }

        let last_block_num = last_block.unwrap();
        let current_block = self
            .client
            .blocks()
            .at_latest()
            .await
            .map_err(|e| Error::custom(e.to_string()))?
            .number();
        Ok(current_block > last_block_num)
    }

    pub async fn update_last_block(&self, block_num: u32) -> Result<()> {
        let mut last_block = self.last_processed_block.lock().await;
        *last_block = Some(block_num);
        fs::write(self.data_dir.join(LAST_BLOCK_FILE), block_num.to_string())
            .map_err(|e| Error::custom(e.to_string()))?;
        Ok(())
    }

    pub async fn get_missed_blocks(
        &self,
    ) -> Result<Vec<Block<PolkadotConfig, OnlineClient<PolkadotConfig>>>> {
        let last_block_num = self.last_processed_block.lock().await.unwrap_or(0);
        let current_block = self
            .client
            .blocks()
            .at_latest()
            .await
            .map_err(|e| Error::custom(e.to_string()))?;
        let current_block_num = current_block.number();

        if current_block_num <= last_block_num {
            return Ok(Vec::new());
        }

        let mut missed_blocks = Vec::new();
        let mut block_hash = current_block.hash();

        // Walk backwards from current block to last processed block + 1
        for _ in (last_block_num + 1..=current_block_num).rev() {
            let block = self
                .client
                .blocks()
                .at(block_hash)
                .await
                .map_err(|e| Error::custom(e.to_string()))?;
            block_hash = block.header().parent_hash;
            missed_blocks.push(block);

            // Stop if we've reached the genesis block (parent_hash is zero)
            if block_hash == Default::default() {
                break;
            }
        }

        // Reverse to get chronological order (oldest first)
        missed_blocks.reverse();
        Ok(missed_blocks)
    }
}
