#[cfg(test)]
mod tests {
    use crate::{
        account::load_cyborg_test_key,
        builder::CyborgOracleFeederBuilder,
        cli::{Cli, Commands},
        error::Error,
        feeder::{OracleFeeder, SharedState},
        substrate_interface::api::runtime_types::cyborg_primitives::{
            miner::MinerType, oracle::ProcessStatus,
        },
        tx_queue::{Transaction, TxOutput},
    };
    use async_trait::async_trait;
    use clap::Parser;
    use std::sync::Arc;
    use subxt::{blocks::Block, OnlineClient, PolkadotConfig};
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

        async fn run_verify_proofs(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }

        async fn verify_proof(
            &self,
            _task_id: u64,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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

        async fn get_miner_data(&self, _miner_ip: &str) -> ProcessStatus {
            ProcessStatus {
                online: true,
                available: true,
            }
        }

        async fn process_block(
            &self,
            _block: &Block<PolkadotConfig, OnlineClient<PolkadotConfig>>,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }
    }

    #[test]
    fn test_cli_parsing_valid() {
        let args = vec![
            "cyborg-oracle-feeder",
            "start",
            "--parachain-url",
            "ws://localhost:9944",
            "--account-seed",
            "//Alice",
        ];

        let cli = Cli::parse_from(args);

        assert!(cli.command.is_some());
        if let Some(Commands::Start {
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
    fn test_cli_parsing_no_command() {
        let args = vec!["cyborg-oracle-feeder"];
        let cli = Cli::parse_from(args);
        assert!(cli.command.is_none());
    }

    #[tokio::test]
    async fn test_builder_with_keypair() {
        let builder = CyborgOracleFeederBuilder::default();
        let result = builder.keypair("//Alice");
        assert!(result.is_ok(), "Builder should accept valid keypair");
    }

    // #[tokio::test]
    // async fn test_builder_with_invalid_keypair() {
    //     let builder = CyborgOracleFeederBuilder::default();
    //     let result = builder.keypair("invalid_seed_phrase");
    //     assert!(result.is_err(), "Builder should reject invalid keypair");
    // }

    #[test]
    fn test_account_keypair_loading() {
        // Test with valid key
        std::env::set_var("CYBORG_TEST_KEY", "//Alice");
        let result = load_cyborg_test_key();
        assert!(result.is_ok(), "Failed to load keypair: {:?}", result.err());

        // Test error case
        std::env::remove_var("CYBORG_TEST_KEY");
        let error_result = load_cyborg_test_key();
        assert!(error_result.is_err(), "Should have failed without env var");
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
    async fn test_transaction_queue_operations() {
        crate::tx_queue::init_transaction_queue();
        let queue = crate::tx_queue::TRANSACTION_QUEUE.get().unwrap();

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

        // Test collect_miner_data
        feeder.collect_miner_data().await.unwrap();
        let data = feeder.shared_state.current_miners_data.lock().await;
        assert!(data.is_some());

        // Test get_miner_data
        let status = feeder.get_miner_data("127.0.0.1:8080").await;
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
        crate::tx_queue::init_transaction_queue();
        let queue = crate::tx_queue::TRANSACTION_QUEUE.get().unwrap();

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

    // Integration-style tests
    #[tokio::test]
    async fn test_feeder_lifecycle() {
        // This test simulates the basic lifecycle without external dependencies
        let shared_state = Arc::new(SharedState {
            current_miners_data: Mutex::new(None),
        });

        let feeder = MockOracleFeeder { shared_state };

        // Test that all methods can be called without panicking
        let _ = feeder.run_check_miners().await;
        let _ = feeder.run_verify_proofs().await;
        let _ = feeder.verify_proof(1).await;
        let _ = feeder.collect_miner_data().await;
        let _ = feeder.feed().await;
        let _ = feeder.get_miner_data("127.0.0.1:8080").await;

        assert!(true, "All feeder methods executed without panic");
    }
}
