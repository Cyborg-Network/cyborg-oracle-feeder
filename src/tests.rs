#[cfg(test)]
mod tests {
    use crate::{
        account::load_cyborg_test_key,
        error::Error,
        feeder::{OracleFeeder, SharedState},
        substrate_interface::api::runtime_types::cyborg_primitives::{
            miner::MinerType, oracle::ProcessStatus,
        },
        tx_queue::{Transaction, TxOutput},
    };
    use async_trait::async_trait;
    use reqwest::Client;
    use std::sync::Arc;
    use tempfile::tempdir;
    use tokio::sync::Mutex;

    // Mock OracleFeeder for testing
    struct MockOracleFeeder {
        pub shared_state: Arc<SharedState>,
    }

    #[async_trait]
    impl OracleFeeder for MockOracleFeeder {
        async fn run_check_miners(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }

        async fn collect_miner_data(&self) -> Result<(), subxt::Error> {
            let mut miner_data_guard = self.shared_state.current_miners_data.lock().await;
            *miner_data_guard = Some(Vec::new());
            Ok(())
        }

        async fn feed(&self) -> Result<(), Box<dyn std::error::Error>> {
            Ok(())
        }

        async fn get_miner_data(&self, _miner_ip: &str, _reqwest_client: &Client) -> ProcessStatus {
            ProcessStatus {
                online: true,
                available: true,
            }
        }
    }

    #[test]
    fn test_transaction_creation() {
        let executor: crate::tx_queue::TxExecutor =
            Box::new(|| Box::pin(async { Ok(TxOutput::OracleFeedSuccess) }));

        let transaction = Transaction::new(executor, None);
        assert_eq!(transaction.retry_count, 0);
        assert!(transaction.responder.is_none());
    }

    #[test]
    fn test_transaction_retry_increment() {
        let executor: crate::tx_queue::TxExecutor =
            Box::new(|| Box::pin(async { Ok(TxOutput::OracleFeedSuccess) }));

        let mut transaction = Transaction::new(executor, None);
        assert_eq!(transaction.retry_count(), 0);

        transaction.increment_retry();
        assert_eq!(transaction.retry_count(), 1);
    }

    #[tokio::test]
    async fn test_transaction_queue_initialization() {
        // Create a fresh transaction queue for this test
        let queue = crate::tx_queue::TransactionQueue::new();

        // Queue should be empty after initialization
        let inner_queue = queue.inner.lock().await;
        assert!(inner_queue.is_empty());
    }

    #[tokio::test]
    async fn test_transaction_enqueue() {
        // Create a fresh transaction queue for this test
        let queue = crate::tx_queue::TransactionQueue::new();

        let rx = queue
            .enqueue(|| async { Ok(TxOutput::OracleFeedSuccess) })
            .await
            .unwrap();

        let result = rx.await.unwrap();
        assert!(matches!(result, Ok(TxOutput::OracleFeedSuccess)));
    }

    #[test]
    fn test_process_status_clone() {
        let status = ProcessStatus {
            online: true,
            available: false,
        };

        let cloned = status.clone();
        assert_eq!(status.online, cloned.online);
        assert_eq!(status.available, cloned.available);
    }

    #[test]
    fn test_miner_type_clone() {
        let cloud = MinerType::Cloud;
        let edge = MinerType::Edge;

        assert!(matches!(cloud.clone(), MinerType::Cloud));
        assert!(matches!(edge.clone(), MinerType::Edge));
    }

    #[test]
    fn test_error_creation() {
        let custom_error = Error::custom("test error");
        assert!(matches!(custom_error, Error::Custom(_)));

        let str_error: Error = "test string error".into();
        assert!(matches!(str_error, Error::Custom(_)));
    }

    #[tokio::test]
    async fn test_mock_feeder_operations() {
        let feeder = MockOracleFeeder {
            shared_state: Arc::new(SharedState {
                current_miners_data: Mutex::new(None),
            }),
        };

        let client = Client::new();

        // Test collect_miner_data
        feeder.collect_miner_data().await.unwrap();
        let data = feeder.shared_state.current_miners_data.lock().await;
        assert!(data.is_some());

        // Test get_miner_data
        let status = feeder
            .get_miner_data(&"127.0.0.1:8080".to_string(), &client)
            .await;
        assert!(status.online);
        assert!(status.available);

        // Test feed (should not panic)
        feeder.feed().await.unwrap();
    }

    #[test]
    fn test_block_tracker_data_dir_creation() {
        let temp_dir = tempdir().unwrap();
        let data_dir = temp_dir.path().to_path_buf();

        // Test that we can create the directory structure
        assert!(!data_dir.join("last_block.txt").exists());
    }

    #[test]
    fn test_cli_parsing() {
        use clap::Parser;

        let args = vec![
            "cyborg-oracle-feeder",
            "start",
            "--parachain-url",
            "ws://localhost:9944",
            "--account-seed",
            "//Alice",
        ];
        let cli = crate::cli::Cli::parse_from(args);

        if let Some(crate::cli::Commands::Start {
            parachain_url,
            account_seed,
        }) = cli.command
        {
            assert_eq!(parachain_url, "ws://localhost:9944");
            assert_eq!(account_seed, "//Alice");
        } else {
            panic!("Failed to parse start command");
        }
    }

    #[test]
    fn test_account_keypair_loading() {
        // Set up the environment variable for the test
        std::env::set_var("CYBORG_TEST_KEY", "//Alice");

        let result = load_cyborg_test_key();
        assert!(result.is_ok(), "Failed to load keypair: {:?}", result.err());

        // Test error case by removing the env var
        std::env::remove_var("CYBORG_TEST_KEY");
        let error_result = load_cyborg_test_key();
        assert!(error_result.is_err(), "Should have failed without env var");

        // Test invalid seed format
        std::env::set_var("CYBORG_TEST_KEY", "invalid_seed_format");
        let invalid_result = load_cyborg_test_key();
        assert!(
            invalid_result.is_err(),
            "Should have failed with invalid seed"
        );
    }

    #[tokio::test]
    async fn test_transaction_queue_processing_flag() {
        // Create a fresh transaction queue for this test
        let queue = crate::tx_queue::TransactionQueue::new();

        let rx = queue
            .enqueue(|| async {
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                Ok(TxOutput::OracleFeedSuccess)
            })
            .await
            .unwrap();

        let result = rx.await.unwrap();
        assert!(matches!(result, Ok(TxOutput::OracleFeedSuccess)));
    }

    #[test]
    fn test_transaction_output_debug() {
        let output = TxOutput::OracleFeedSuccess;
        // This should not panic
        let _ = format!("{:?}", output);
    }

    #[tokio::test]
    async fn test_transaction_queue_multiple_operations() {
        let queue = crate::tx_queue::TransactionQueue::new();

        // Enqueue multiple transactions
        let rx1 = queue
            .enqueue(|| async { Ok(TxOutput::OracleFeedSuccess) })
            .await
            .unwrap();

        let rx2 = queue
            .enqueue(|| async {
                tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
                Ok(TxOutput::OracleFeedSuccess)
            })
            .await
            .unwrap();

        let (result1, result2) = tokio::join!(rx1, rx2);
        assert!(matches!(result1.unwrap(), Ok(TxOutput::OracleFeedSuccess)));
        assert!(matches!(result2.unwrap(), Ok(TxOutput::OracleFeedSuccess)));
    }
}
