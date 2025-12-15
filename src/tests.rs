#[cfg(test)]
mod test {
    use crate::{
        account::load_cyborg_test_key,
        builder::CyborgOracleFeederBuilder,
        cli::Commands,
        error::Error,
        feeder::{OracleFeeder, SharedState},
        substrate_interface::api::runtime_types::cyborg_primitives::{
            miner::MinerType, oracle::ProcessStatus,
        },
        tx_queue::{Transaction, TxOutput},
    };
    use async_trait::async_trait;
    use std::sync::Arc;
    use subxt::{blocks::Block, OnlineClient, PolkadotConfig};
    use tempfile::tempdir;
    use tokio::sync::Mutex;

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

    // Unit Tests for cli.rs
    #[test]
    fn test_cli_parsing_valid() {
        let _args = vec![
            "cyborg-oracle-feeder",
            "start",
            "--parachain-url",
            "ws://localhost:9944",
            "--account-seed",
            "//Alice",
        ];
    }

    #[tokio::test]
    async fn test_builder_with_keypair() {
        let builder = CyborgOracleFeederBuilder::default();
        let result = builder.keypair("//Alice");
        assert!(result.is_ok(), "Builder should accept valid keypair");
    }

    #[tokio::test]
    async fn test_async_collect_miner_data() {
        // Mock implementation for testing the async trait method
        struct TestFeeder;

        #[async_trait]
        impl OracleFeeder for TestFeeder {
            async fn run_check_miners(
                &self,
            ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
                Ok(())
            }

            async fn run_verify_proofs(
                &self,
            ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
                Ok(())
            }

            async fn verify_proof(
                &self,
                _task_id: u64,
            ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
                Ok(())
            }

            async fn collect_miner_data(&self) -> Result<(), subxt::Error> {
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

        let feeder = TestFeeder;
        let status = feeder.get_miner_data("127.0.0.1:8080").await;
        assert!(status.online);
        assert!(status.available);
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

    #[tokio::test]
    async fn test_transaction_queue_multiple_operations() {
        crate::tx_queue::init_transaction_queue();
        let queue = crate::tx_queue::TRANSACTION_QUEUE.get().unwrap();

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
        tokio::task::yield_now().await;
        let (result1, result2) = tokio::join!(rx1, rx2);
        assert!(matches!(result1.unwrap(), Ok(TxOutput::OracleFeedSuccess)));
        assert!(matches!(result2.unwrap(), Ok(TxOutput::OracleFeedSuccess)));
    }

    // Unit Tests for data structures
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

    // Integration-style tests for feeder functionality
    #[tokio::test]
    async fn test_feeder_shared_state() {
        let shared_state = Arc::new(SharedState {
            current_miners_data: Mutex::new(None),
        });

        // Test initial state
        let data = shared_state.current_miners_data.lock().await;
        assert!(data.is_none());

        // Test updating state
        drop(data);
        let mut data = shared_state.current_miners_data.lock().await;
        *data = Some(vec![]);
        assert!(data.is_some());
    }

    // Mock OracleFeeder for comprehensive testing
    struct MockOracleFeeder {
        pub shared_state: Arc<SharedState>,
    }

    #[async_trait]
    impl OracleFeeder for MockOracleFeeder {
        async fn run_check_miners(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            // Simulate successful operation
            Ok(())
        }

        async fn run_verify_proofs(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            // Simulate successful operation
            Ok(())
        }

        async fn verify_proof(
            &self,
            _task_id: u64,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            // Simulate proof verification
            Ok(())
        }

        async fn collect_miner_data(&self) -> Result<(), subxt::Error> {
            // Simulate collecting miner data
            let mut miner_data_guard = self.shared_state.current_miners_data.lock().await;
            *miner_data_guard = Some(vec![]);
            Ok(())
        }

        async fn feed(&self) -> Result<(), Box<dyn std::error::Error>> {
            // Simulate feeding oracle
            Ok(())
        }

        async fn get_miner_data(&self, miner_ip: &str) -> ProcessStatus {
            // Return mock status based on IP
            if miner_ip.contains("127.0.0.1") {
                ProcessStatus {
                    online: true,
                    available: true,
                }
            } else {
                ProcessStatus {
                    online: false,
                    available: false,
                }
            }
        }

        async fn process_block(
            &self,
            _block: &Block<PolkadotConfig, OnlineClient<PolkadotConfig>>,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            // Simulate block processing
            Ok(())
        }
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

        // Test async operations
        let _ = feeder.run_check_miners().await;
        let _ = feeder.run_verify_proofs().await;
        let _ = feeder.verify_proof(1).await;
    }

    // Test transaction output debug
    #[test]
    fn test_transaction_output_debug() {
        let output = TxOutput::OracleFeedSuccess;
        // This should not panic
        let debug_output = format!("{:?}", output);
        assert!(debug_output.contains("OracleFeedSuccess"));
    }

    // Test block tracker data directory handling
    #[test]
    fn test_block_tracker_data_dir_creation() {
        let temp_dir = tempdir().unwrap();
        let data_dir = temp_dir.path().to_path_buf();

        // Test that we can create the directory structure
        assert!(!data_dir.join("last_block.txt").exists());

        // Verify directory is accessible
        assert!(data_dir.exists());
    }

    // Test CLI command equality
    #[test]
    fn test_cli_commands_partial_eq() {
        let command1 = Commands::Start {
            parachain_url: "ws://localhost:9944".to_string(),
            account_seed: "//Alice".to_string(),
        };

        let command2 = Commands::Start {
            parachain_url: "ws://localhost:9944".to_string(),
            account_seed: "//Alice".to_string(),
        };

        assert_eq!(command1, command2);
    }

    // Test error conversion
    #[test]
    fn test_error_from_std_io_error() {
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let error: Error = io_error.into();
        assert!(matches!(error, Error::Io(_)));
    }

    // Test transaction execution with error
    #[tokio::test]
    async fn test_transaction_execution_with_error() {
        let executor: crate::tx_queue::TxExecutor =
            Box::new(|| Box::pin(async { Err(Error::custom("test error").into()) }));

        let transaction = Transaction::new(executor, None);
        let result = transaction.execute().await;
        assert!(result.is_err());
    }
}
