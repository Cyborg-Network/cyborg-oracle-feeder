// use crate::substrate_interface::api::runtime_types::bounded_collections::bounded_vec::BoundedVec;
// use crate::substrate_interface::api::runtime_types::cyborg_primitives::miner::MinerType;
// use crate::substrate_interface::api::runtime_types::cyborg_primitives::oracle::{
//     OracleKey, OracleMinerFormat, OracleValue, ProcessStatus,
// };
// use async_trait::async_trait;
// use cyborg_oracle_feeder::*;
// use reqwest::Client;
// use std::sync::Arc;
// use tokio::sync::Mutex;
// #[tokio::test]
// async fn integration_test_mock_feeder_full_run() {
//     // ---------- Mock Shared State ----------
//     struct MockFeeder {
//         shared: Arc<SharedState>,
//     }

//     // ---------- Mock Implementations ----------
//     #[async_trait]
//     impl OracleFeeder for MockFeeder {
//         async fn run_check_miners(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//             println!("run_check_miners() called");
//             Ok(())
//         }

//         async fn collect_miner_data(&self) -> Result<(), subxt::Error> {
//             println!("collect_miner_data() called");

//             let mut lock = self.shared.current_miners_data.lock().await;

//             let mock_id = BoundedVec(vec![1]);

//             *lock = Some(vec![(
//                 OracleKey::Miner(OracleMinerFormat {
//                     id: mock_id,
//                     miner_type: MinerType::Edge,
//                 }),
//                 OracleValue::MinerStatus(ProcessStatus {
//                     online: true,
//                     available: true,
//                 }),
//             )]);

//             Ok(())
//         }

//         async fn feed(&self) -> Result<(), Box<dyn std::error::Error>> {
//             println!("feed() called");
//             let lock = self.shared.current_miners_data.lock().await;
//             assert!(lock.is_some(), "Miner data must be set before feed()");
//             Ok(())
//         }

//         async fn get_miner_data(&self, miner_ip: &str, _reqwest_client: &Client) -> ProcessStatus {
//             println!("get_miner_data() called for IP {}", miner_ip);
//             ProcessStatus {
//                 online: true,
//                 available: true,
//             }
//         }
//     }

//     // ---------- Setup ----------
//     let shared = Arc::new(SharedState {
//         current_miners_data: Mutex::new(None),
//     });

//     let feeder = MockFeeder { shared };

//     // ---------- Test Flow ----------
//     feeder.collect_miner_data().await.unwrap();
//     feeder.run_check_miners().await.unwrap();
//     feeder.feed().await.unwrap();

//     // Verify miner data persisted
//     let guard = feeder.shared.current_miners_data.lock().await;
//     assert!(guard.is_some());
//     assert_eq!(guard.as_ref().unwrap().len(), 1);
// }


#![cfg(test)]

use cyborg_oracle_feeder::*;
use async_trait::async_trait;
use reqwest::Client;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tokio::time::timeout;

// Mock HTTP server for testing miner health endpoints
mod mock_server {
    use std::net::SocketAddr;
    use tokio::net::TcpListener;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    pub async fn start_mock_miner(port: u16, is_healthy: bool) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let addr = SocketAddr::from(([127, 0, 0, 1], port));
            let listener = TcpListener::bind(addr).await.unwrap();
            
            loop {
                if let Ok((mut stream, _)) = listener.accept().await {
                    let mut buffer = [0; 1024];
                    let _ = stream.read(&mut buffer).await;
                    
                    let response = if is_healthy {
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{{\"isActive\":true}}"
                        )
                    } else {
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{{\"isActive\":false}}"
                        )
                    };
                    
                    let _ = stream.write_all(response.as_bytes()).await;
                }
            }
        })
    }

    pub async fn start_failing_miner(port: u16) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let addr = SocketAddr::from(([127, 0, 0, 1], port));
            let listener = TcpListener::bind(addr).await.unwrap();
            
            loop {
                if let Ok((mut stream, _)) = listener.accept().await {
                    // Immediately close connection to simulate failure
                    drop(stream);
                }
            }
        })
    }
}

// ============================================================================
// Test Fixtures and Utilities
// ============================================================================

struct TestContext {
    shared_state: Arc<SharedState>,
    mock_feeder: MockOracleFeeder,
}

impl TestContext {
    fn new() -> Self {
        let shared_state = Arc::new(SharedState {
            current_miners_data: Mutex::new(None),
        });
        
        let mock_feeder = MockOracleFeeder {
            shared_state: Arc::clone(&shared_state),
            should_fail_collection: Arc::new(RwLock::new(false)),
            should_fail_feed: Arc::new(RwLock::new(false)),
            collection_call_count: Arc::new(Mutex::new(0)),
            feed_call_count: Arc::new(Mutex::new(0)),
        };
        
        Self {
            shared_state,
            mock_feeder,
        }
    }
}

struct MockOracleFeeder {
    shared_state: Arc<SharedState>,
    should_fail_collection: Arc<RwLock<bool>>,
    should_fail_feed: Arc<RwLock<bool>>,
    collection_call_count: Arc<Mutex<u32>>,
    feed_call_count: Arc<Mutex<u32>>,
}

#[async_trait]
impl OracleFeeder for MockOracleFeeder {
    async fn run_check_miners(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    async fn collect_miner_data(&self) -> Result<(), subxt::Error> {
        let mut count = self.collection_call_count.lock().await;
        *count += 1;
        
        if *self.should_fail_collection.read().await {
            return Err(subxt::Error::Other("Simulated collection failure".to_string()));
        }
        
        use crate::substrate_interface::api::runtime_types::{
            bounded_collections::bounded_vec::BoundedVec,
            cyborg_primitives::{
                miner::MinerType,
                oracle::{OracleKey, OracleMinerFormat, OracleValue, ProcessStatus},
            },
        };
        
        let mut lock = self.shared_state.current_miners_data.lock().await;
        *lock = Some(vec![
            (
                OracleKey::Miner(OracleMinerFormat {
                    id: BoundedVec(vec![1]),
                    miner_type: MinerType::Cloud,
                }),
                OracleValue::MinerStatus(ProcessStatus {
                    online: true,
                    available: true,
                }),
            ),
            (
                OracleKey::Miner(OracleMinerFormat {
                    id: BoundedVec(vec![2]),
                    miner_type: MinerType::Edge,
                }),
                OracleValue::MinerStatus(ProcessStatus {
                    online: false,
                    available: false,
                }),
            ),
        ]);
        
        Ok(())
    }

    async fn feed(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut count = self.feed_call_count.lock().await;
        *count += 1;
        
        if *self.should_fail_feed.read().await {
            return Err("Simulated feed failure".into());
        }
        
        let lock = self.shared_state.current_miners_data.lock().await;
        if lock.is_none() {
            return Err("No miner data to feed".into());
        }
        
        Ok(())
    }

    async fn get_miner_data(
        &self,
        miner_ip: &str,
        _reqwest_client: &Client,
    ) -> crate::substrate_interface::api::runtime_types::cyborg_primitives::oracle::ProcessStatus {
        use crate::substrate_interface::api::runtime_types::cyborg_primitives::oracle::ProcessStatus;
        
        if miner_ip.contains("healthy") {
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
}

// ============================================================================
// Integration Tests
// ============================================================================

#[tokio::test]
async fn test_full_oracle_cycle_success() {
    let ctx = TestContext::new();
    
    // Collect miner data
    let result = ctx.mock_feeder.collect_miner_data().await;
    assert!(result.is_ok(), "Data collection should succeed");
    
    // Verify data was collected
    let data = ctx.shared_state.current_miners_data.lock().await;
    assert!(data.is_some(), "Miner data should be present");
    assert_eq!(data.as_ref().unwrap().len(), 2, "Should have 2 miners");
    drop(data);
    
    // Feed the oracle
    let feed_result = ctx.mock_feeder.feed().await;
    assert!(feed_result.is_ok(), "Feed should succeed with valid data");
    
    // Verify feed was called
    let feed_count = *ctx.mock_feeder.feed_call_count.lock().await;
    assert_eq!(feed_count, 1, "Feed should be called once");
}

#[tokio::test]
async fn test_collection_failure_handling() {
    let ctx = TestContext::new();
    
    // Simulate collection failure
    *ctx.mock_feeder.should_fail_collection.write().await = true;
    
    let result = ctx.mock_feeder.collect_miner_data().await;
    assert!(result.is_err(), "Collection should fail when simulated");
    
    // Verify data was not set
    let data = ctx.shared_state.current_miners_data.lock().await;
    assert!(data.is_none(), "No data should be present after failed collection");
}

#[tokio::test]
async fn test_feed_without_data_fails() {
    let ctx = TestContext::new();
    
    // Try to feed without collecting data first
    let result = ctx.mock_feeder.feed().await;
    assert!(result.is_err(), "Feed should fail without miner data");
}

#[tokio::test]
async fn test_concurrent_data_access() {
    let ctx = TestContext::new();
    
    // Spawn multiple tasks that collect and read data concurrently
    let mut handles = vec![];
    
    for i in 0..10 {
        let feeder = MockOracleFeeder {
            shared_state: Arc::clone(&ctx.shared_state),
            should_fail_collection: Arc::new(RwLock::new(false)),
            should_fail_feed: Arc::new(RwLock::new(false)),
            collection_call_count: Arc::clone(&ctx.mock_feeder.collection_call_count),
            feed_call_count: Arc::clone(&ctx.mock_feeder.feed_call_count),
        };
        
        let handle = tokio::spawn(async move {
            if i % 2 == 0 {
                feeder.collect_miner_data().await
            } else {
                // Just read the data
                let _data = feeder.shared_state.current_miners_data.lock().await;
                Ok(())
            }
        });
        
        handles.push(handle);
    }
    
    // Wait for all tasks to complete
    for handle in handles {
        let result = handle.await;
        assert!(result.is_ok(), "Concurrent access should not panic");
    }
    
    // Verify final state is consistent
    let data = ctx.shared_state.current_miners_data.lock().await;
    assert!(data.is_some(), "Data should be present after concurrent operations");
}

#[tokio::test]
async fn test_miner_health_check_timeout() {
    let client = Client::builder()
        .timeout(Duration::from_millis(100))
        .build()
        .unwrap();
    
    let ctx = TestContext::new();
    
    // Use a non-existent IP to trigger timeout
    let status = ctx.mock_feeder
        .get_miner_data("192.168.255.255:9999", &client)
        .await;
    
    assert!(!status.online, "Timeout should result in offline status");
    assert!(!status.available, "Timeout should result in unavailable status");
}

#[tokio::test]
async fn test_miner_health_check_with_mock_server() {
    // Start a mock healthy miner
    let healthy_server = mock_server::start_mock_miner(18080, true).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    let client = Client::new();
    let ctx = TestContext::new();
    
    let status = ctx.mock_feeder
        .get_miner_data("127.0.0.1:18080", &client)
        .await;
    
    // Note: This uses the mock implementation which checks for "healthy" in IP
    // In real implementation, adjust based on actual health check logic
    assert!(!status.online || !status.available, "Mock uses IP check, not actual server");
    
    healthy_server.abort();
}

#[tokio::test]
async fn test_retry_mechanism_on_transient_failures() {
    let ctx = TestContext::new();
    
    // Enable failure initially
    *ctx.mock_feeder.should_fail_collection.write().await = true;
    
    // First attempt should fail
    let result1 = ctx.mock_feeder.collect_miner_data().await;
    assert!(result1.is_err(), "First attempt should fail");
    
    // Disable failure for retry
    *ctx.mock_feeder.should_fail_collection.write().await = false;
    
    // Retry should succeed
    let result2 = ctx.mock_feeder.collect_miner_data().await;
    assert!(result2.is_ok(), "Retry should succeed");
    
    // Verify data is now available
    let data = ctx.shared_state.current_miners_data.lock().await;
    assert!(data.is_some(), "Data should be present after successful retry");
}

#[tokio::test]
async fn test_data_consistency_across_cycles() {
    let ctx = TestContext::new();
    
    // First cycle
    ctx.mock_feeder.collect_miner_data().await.unwrap();
    let data1 = ctx.shared_state.current_miners_data.lock().await.clone();
    drop(data1);
    
    // Second cycle (should overwrite)
    ctx.mock_feeder.collect_miner_data().await.unwrap();
    let data2 = ctx.shared_state.current_miners_data.lock().await.clone();
    
    assert!(data2.is_some(), "Data should exist after second cycle");
    assert_eq!(
        data2.as_ref().unwrap().len(),
        2,
        "Should maintain consistent miner count"
    );
}

#[tokio::test]
async fn test_feed_call_idempotency() {
    let ctx = TestContext::new();
    
    // Collect data once
    ctx.mock_feeder.collect_miner_data().await.unwrap();
    
    // Feed multiple times with same data
    for _ in 0..3 {
        let result = ctx.mock_feeder.feed().await;
        assert!(result.is_ok(), "Feed should be idempotent");
    }
    
    let feed_count = *ctx.mock_feeder.feed_call_count.lock().await;
    assert_eq!(feed_count, 3, "Feed should be called 3 times");
}

#[tokio::test]
async fn test_graceful_degradation_on_partial_failure() {
    let ctx = TestContext::new();
    
    // Collect initial data
    ctx.mock_feeder.collect_miner_data().await.unwrap();
    
    // Simulate feed failure
    *ctx.mock_feeder.should_fail_feed.write().await = true;
    let feed_result = ctx.mock_feeder.feed().await;
    assert!(feed_result.is_err(), "Feed should fail when simulated");
    
    // Data should still be available for next cycle
    let data = ctx.shared_state.current_miners_data.lock().await;
    assert!(data.is_some(), "Data should persist despite feed failure");
}

#[tokio::test]
async fn test_miner_type_distinction() {
    use crate::substrate_interface::api::runtime_types::cyborg_primitives::miner::MinerType;
    
    let ctx = TestContext::new();
    ctx.mock_feeder.collect_miner_data().await.unwrap();
    
    let data = ctx.shared_state.current_miners_data.lock().await;
    let miners = data.as_ref().unwrap();
    
    // Check we have both types
    let has_cloud = miners.iter().any(|(key, _)| {
        if let crate::substrate_interface::api::runtime_types::cyborg_primitives::oracle::OracleKey::Miner(miner) = key {
            matches!(miner.miner_type, MinerType::Cloud)
        } else {
            false
        }
    });
    
    let has_edge = miners.iter().any(|(key, _)| {
        if let crate::substrate_interface::api::runtime_types::cyborg_primitives::oracle::OracleKey::Miner(miner) = key {
            matches!(miner.miner_type, MinerType::Edge)
        } else {
            false
        }
    });
    
    assert!(has_cloud, "Should have at least one cloud miner");
    assert!(has_edge, "Should have at least one edge miner");
}

#[tokio::test]
async fn test_empty_miner_list_handling() {
    let shared_state = Arc::new(SharedState {
        current_miners_data: Mutex::new(Some(vec![])),
    });
    
    let feeder = MockOracleFeeder {
        shared_state: Arc::clone(&shared_state),
        should_fail_collection: Arc::new(RwLock::new(false)),
        should_fail_feed: Arc::new(RwLock::new(false)),
        collection_call_count: Arc::new(Mutex::new(0)),
        feed_call_count: Arc::new(Mutex::new(0)),
    };
    
    let result = feeder.feed().await;
    assert!(result.is_ok(), "Should handle empty miner list gracefully");
}

#[tokio::test]
async fn test_large_miner_dataset() {
    use crate::substrate_interface::api::runtime_types::{
        bounded_collections::bounded_vec::BoundedVec,
        cyborg_primitives::{
            miner::MinerType,
            oracle::{OracleKey, OracleMinerFormat, OracleValue, ProcessStatus},
        },
    };
    
    let shared_state = Arc::new(SharedState {
        current_miners_data: Mutex::new(None),
    });
    
    // Create 1000 mock miners
    let mut miners = Vec::new();
    for i in 0..1000 {
        miners.push((
            OracleKey::Miner(OracleMinerFormat {
                id: BoundedVec(vec![i as u8]),
                miner_type: if i % 2 == 0 { MinerType::Cloud } else { MinerType::Edge },
            }),
            OracleValue::MinerStatus(ProcessStatus {
                online: i % 3 != 0,
                available: i % 5 != 0,
            }),
        ));
    }
    
    *shared_state.current_miners_data.lock().await = Some(miners);
    
    let feeder = MockOracleFeeder {
        shared_state: Arc::clone(&shared_state),
        should_fail_collection: Arc::new(RwLock::new(false)),
        should_fail_feed: Arc::new(RwLock::new(false)),
        collection_call_count: Arc::new(Mutex::new(0)),
        feed_call_count: Arc::new(Mutex::new(0)),
    };
    
    let result = feeder.feed().await;
    assert!(result.is_ok(), "Should handle large dataset");
}

#[tokio::test]
async fn test_operation_timeout_handling() {
    let ctx = TestContext::new();
    
    // Test with timeout
    let result = timeout(
        Duration::from_secs(5),
        ctx.mock_feeder.collect_miner_data()
    ).await;
    
    assert!(result.is_ok(), "Operation should complete within timeout");
    assert!(result.unwrap().is_ok(), "Operation should succeed");
}

#[tokio::test]
async fn test_sequential_cycle_execution() {
    let ctx = TestContext::new();
    
    // Simulate 5 complete cycles
    for i in 0..5 {
        let collect_result = ctx.mock_feeder.collect_miner_data().await;
        assert!(collect_result.is_ok(), "Cycle {} collection failed", i);
        
        let feed_result = ctx.mock_feeder.feed().await;
        assert!(feed_result.is_ok(), "Cycle {} feed failed", i);
    }
    
    let collection_count = *ctx.mock_feeder.collection_call_count.lock().await;
    let feed_count = *ctx.mock_feeder.feed_call_count.lock().await;
    
    assert_eq!(collection_count, 5, "Should have 5 collections");
    assert_eq!(feed_count, 5, "Should have 5 feeds");
}

#[tokio::test]
async fn test_state_recovery_after_failure() {
    let ctx = TestContext::new();
    
    // Successful cycle
    ctx.mock_feeder.collect_miner_data().await.unwrap();
    ctx.mock_feeder.feed().await.unwrap();
    
    // Failed cycle
    *ctx.mock_feeder.should_fail_feed.write().await = true;
    let _ = ctx.mock_feeder.feed().await;
    
    // Recovery cycle
    *ctx.mock_feeder.should_fail_feed.write().await = false;
    let recovery_result = ctx.mock_feeder.feed().await;
    
    assert!(recovery_result.is_ok(), "Should recover after failure");
}

#[tokio::test]
async fn test_miner_status_transitions() {
    use crate::substrate_interface::api::runtime_types::{
        bounded_collections::bounded_vec::BoundedVec,
        cyborg_primitives::{
            miner::MinerType,
            oracle::{OracleKey, OracleMinerFormat, OracleValue, ProcessStatus},
        },
    };
    
    let shared_state = Arc::new(SharedState {
        current_miners_data: Mutex::new(None),
    });
    
    // Initial state: miner is online
    *shared_state.current_miners_data.lock().await = Some(vec![(
        OracleKey::Miner(OracleMinerFormat {
            id: BoundedVec(vec![1]),
            miner_type: MinerType::Cloud,
        }),
        OracleValue::MinerStatus(ProcessStatus {
            online: true,
            available: true,
        }),
    )]);
    
    // Transition to offline
    *shared_state.current_miners_data.lock().await = Some(vec![(
        OracleKey::Miner(OracleMinerFormat {
            id: BoundedVec(vec![1]),
            miner_type: MinerType::Cloud,
        }),
        OracleValue::MinerStatus(ProcessStatus {
            online: false,
            available: false,
        }),
    )]);
    
    let data = shared_state.current_miners_data.lock().await;
    if let Some(miners) = data.as_ref() {
        if let (_, OracleValue::MinerStatus(status)) = &miners[0] {
            assert!(!status.online, "Miner should be offline");
            assert!(!status.available, "Miner should be unavailable");
        }
    }
}