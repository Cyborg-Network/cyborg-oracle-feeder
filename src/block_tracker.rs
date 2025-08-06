use crate::error::Result;
use std::{fs, path::PathBuf};
use subxt::{blocks::Block, OnlineClient, PolkadotConfig};

const BLOCK_TRACKER_FILE: &str = "last_block.txt";

pub struct BlockTracker {
    file_path: PathBuf,
}

impl BlockTracker {
    pub fn new(data_dir: Option<PathBuf>) -> Result<Self> {
        let file_path = match data_dir {
            Some(dir) => dir.join(BLOCK_TRACKER_FILE),
            None => PathBuf::from(BLOCK_TRACKER_FILE),
        };

        Ok(Self { file_path })
    }

    pub async fn get_last_block(&self) -> Result<Option<String>> {
        if !self.file_path.exists() {
            return Ok(None);
        }

        let block_hash = fs::read_to_string(&self.file_path)?;
        Ok(Some(block_hash.trim().to_string()))
    }

    pub async fn update_last_block(
        &self,
        block: &Block<PolkadotConfig, OnlineClient<PolkadotConfig>>,
    ) -> Result<()> {
        let block_hash = hex::encode(block.hash());
        fs::write(&self.file_path, block_hash)?;
        Ok(())
    }

    pub fn is_first_run(&self) -> bool {
        !self.file_path.exists()
    }
}
