use crate::substrate_interface::api::runtime_types::bounded_collections::bounded_vec::BoundedVec;
use crate::substrate_interface::api::runtime_types::cyborg_primitives::miner::MinerType;
use crate::substrate_interface::api::runtime_types::cyborg_primitives::oracle::{
    OracleKey, OracleMinerFormat, OracleValue, ProcessStatus,
};
use async_trait::async_trait;
use cyborg_oracle_feeder::*;
use std::sync::Arc;
use subxt::{OnlineClient, PolkadotConfig};
use tokio::sync::Mutex;
#[tokio::test]
async fn integration_test_mock_feeder_full_run() {
    // ---------- Mock Shared State ----------
    struct MockFeeder {
        shared: Arc<SharedState>,
    }

    // ---------- Mock Implementations ----------
    #[async_trait]
    impl OracleFeeder for MockFeeder {
        async fn run_check_miners(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            println!("run_check_miners() called");
            Ok(())
        }

        async fn run_verify_proofs(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            println!("run_verify_proofs() called");
            Ok(())
        }

        async fn collect_miner_data(&self) -> Result<(), subxt::Error> {
            println!("collect_miner_data() called");

            let mut lock = self.shared.current_miners_data.lock().await;

            let mock_id = BoundedVec(vec![1]);

            *lock = Some(vec![(
                OracleKey::Miner(OracleMinerFormat {
                    id: mock_id,
                    miner_type: MinerType::Edge,
                }),
                OracleValue::MinerStatus(ProcessStatus {
                    online: true,
                    available: true,
                }),
            )]);

            Ok(())
        }

        async fn feed(&self) -> Result<(), Box<dyn std::error::Error>> {
            println!("feed() called");
            let lock = self.shared.current_miners_data.lock().await;
            assert!(lock.is_some(), "Miner data must be set before feed()");
            Ok(())
        }

        async fn verify_proof(
            &self,
            task_id: u64,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            println!("verify_proof() called with task_id={}", task_id);
            Ok(())
        }

        async fn get_miner_data(&self, miner_ip: &str) -> ProcessStatus {
            println!("get_miner_data() called for IP {}", miner_ip);
            ProcessStatus {
                online: true,
                available: true,
            }
        }

        async fn process_block(
            &self,
            _block: &subxt::blocks::Block<PolkadotConfig, OnlineClient<PolkadotConfig>>,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            println!("process_block() called");
            Ok(())
        }
    }

    // ---------- Setup ----------
    let shared = Arc::new(SharedState {
        current_miners_data: Mutex::new(None),
    });

    let feeder = MockFeeder { shared };

    // ---------- Test Flow ----------
    feeder.collect_miner_data().await.unwrap();
    feeder.run_check_miners().await.unwrap();
    feeder.run_verify_proofs().await.unwrap();
    feeder.feed().await.unwrap();

    // Verify miner data persisted
    let guard = feeder.shared.current_miners_data.lock().await;
    assert!(guard.is_some());
    assert_eq!(guard.as_ref().unwrap().len(), 1);
}
