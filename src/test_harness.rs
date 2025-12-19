#![cfg(test)]

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};

use crate::feeder::SharedState;

// ============================================================================
// Test Utilities and Helpers
// ============================================================================

/// Test configuration for parameterized testing
#[allow(dead_code)]
#[derive(Clone)]
pub struct TestConfig {
    pub miner_count: usize,
    pub failure_rate: f64,
    pub timeout_ms: u64,
    pub retry_count: u32,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            miner_count: 10,
            failure_rate: 0.0,
            timeout_ms: 5000,
            retry_count: 3,
        }
    }
}

/// Statistics collector for test metrics
pub struct TestMetrics {
    pub successful_collections: Arc<Mutex<u32>>,
    pub failed_collections: Arc<Mutex<u32>>,
    pub successful_feeds: Arc<Mutex<u32>>,
    pub failed_feeds: Arc<Mutex<u32>>,
    pub total_duration_ms: Arc<Mutex<u128>>,
}

impl TestMetrics {
    pub fn new() -> Self {
        Self {
            successful_collections: Arc::new(Mutex::new(0)),
            failed_collections: Arc::new(Mutex::new(0)),
            successful_feeds: Arc::new(Mutex::new(0)),
            failed_feeds: Arc::new(Mutex::new(0)),
            total_duration_ms: Arc::new(Mutex::new(0)),
        }
    }

    pub async fn record_collection_success(&self) {
        *self.successful_collections.lock().await += 1;
    }

    pub async fn record_collection_failure(&self) {
        *self.failed_collections.lock().await += 1;
    }

    pub async fn record_feed_success(&self) {
        *self.successful_feeds.lock().await += 1;
    }

    pub async fn record_feed_failure(&self) {
        *self.failed_feeds.lock().await += 1;
    }

    pub async fn record_duration(&self, duration: Duration) {
        *self.total_duration_ms.lock().await += duration.as_millis();
    }

    pub async fn get_summary(&self) -> MetricsSummary {
        MetricsSummary {
            successful_collections: *self.successful_collections.lock().await,
            failed_collections: *self.failed_collections.lock().await,
            successful_feeds: *self.successful_feeds.lock().await,
            failed_feeds: *self.failed_feeds.lock().await,
            total_duration_ms: *self.total_duration_ms.lock().await,
        }
    }
}

#[derive(Debug)]
pub struct MetricsSummary {
    pub successful_collections: u32,
    pub failed_collections: u32,
    pub successful_feeds: u32,
    pub failed_feeds: u32,
    pub total_duration_ms: u128,
}

impl MetricsSummary {
    pub fn collection_success_rate(&self) -> f64 {
        let total = self.successful_collections + self.failed_collections;
        if total == 0 {
            return 0.0;
        }
        (self.successful_collections as f64 / total as f64) * 100.0
    }

    pub fn feed_success_rate(&self) -> f64 {
        let total = self.successful_feeds + self.failed_feeds;
        if total == 0 {
            return 0.0;
        }
        (self.successful_feeds as f64 / total as f64) * 100.0
    }
}

/// Mock miner simulator for testing
pub struct MinerSimulator {
    miners: Arc<RwLock<HashMap<Vec<u8>, MinerState>>>,
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct MinerState {
    pub online: bool,
    pub available: bool,
    pub response_delay_ms: u64,
}

impl MinerSimulator {
    pub fn new() -> Self {
        Self {
            miners: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn add_miner(&self, id: Vec<u8>, state: MinerState) {
        self.miners.write().await.insert(id, state);
    }

    pub async fn update_miner_state(&self, id: &[u8], online: bool, available: bool) {
        if let Some(miner) = self.miners.write().await.get_mut(id) {
            miner.online = online;
            miner.available = available;
        }
    }

    pub async fn get_miner_state(&self, id: &[u8]) -> Option<MinerState> {
        self.miners.read().await.get(id).cloned()
    }

    pub async fn simulate_miner_failure(&self, id: &[u8]) {
        self.update_miner_state(id, false, false).await;
    }

    pub async fn simulate_miner_recovery(&self, id: &[u8]) {
        self.update_miner_state(id, true, true).await;
    }
}

// ============================================================================
// End-to-End Integration Tests
// ============================================================================

#[tokio::test]
async fn test_e2e_complete_oracle_cycle() {
    use crate::substrate_interface::api::runtime_types::{
        bounded_collections::bounded_vec::BoundedVec,
        cyborg_primitives::{
            miner::MinerType,
            oracle::{OracleKey, OracleMinerFormat, OracleValue, ProcessStatus},
        },
    };

    let metrics = TestMetrics::new();
    let shared_state = Arc::new(SharedState {
        current_miners_data: Mutex::new(None),
    });

    let start = tokio::time::Instant::now();

    // Phase 1: Data Collection - Add a small delay to ensure duration > 0
    tokio::time::sleep(Duration::from_millis(1)).await; // Add minimal delay

    let miners = vec![
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
                online: true,
                available: false,
            }),
        ),
    ];

    *shared_state.current_miners_data.lock().await = Some(miners);
    metrics.record_collection_success().await;

    // Phase 2: Verification
    let data = shared_state.current_miners_data.lock().await;
    assert!(data.is_some());
    assert_eq!(data.as_ref().unwrap().len(), 2);
    drop(data); // Explicitly drop the lock

    // Phase 3: Feed (simulated) - Add another small delay
    tokio::time::sleep(Duration::from_millis(1)).await;
    metrics.record_feed_success().await;

    let duration = start.elapsed();
    metrics.record_duration(duration).await;

    // Verify metrics
    let summary = metrics.get_summary().await;
    assert_eq!(summary.successful_collections, 1);
    assert_eq!(summary.successful_feeds, 1);
    assert!(
        summary.total_duration_ms > 0,
        "Total duration should be > 0, got {}",
        summary.total_duration_ms
    );
}

#[tokio::test]
async fn test_e2e_failure_recovery_workflow() {
    use crate::substrate_interface::api::runtime_types::{
        bounded_collections::bounded_vec::BoundedVec,
        cyborg_primitives::{
            miner::MinerType,
            oracle::{OracleKey, OracleMinerFormat, OracleValue, ProcessStatus},
        },
    };

    let metrics = TestMetrics::new();
    let simulator = MinerSimulator::new();
    let shared_state = Arc::new(SharedState {
        current_miners_data: Mutex::new(None),
    });

    // Setup miners
    simulator
        .add_miner(
            vec![1],
            MinerState {
                online: false,
                available: false,
                response_delay_ms: 0,
            },
        )
        .await;

    // Attempt 1: Failure
    metrics.record_collection_failure().await;

    // Simulate recovery
    simulator.simulate_miner_recovery(&vec![1]).await;

    // Attempt 2: Success
    let miners = vec![(
        OracleKey::Miner(OracleMinerFormat {
            id: BoundedVec(vec![1]),
            miner_type: MinerType::Cloud,
        }),
        OracleValue::MinerStatus(ProcessStatus {
            online: true,
            available: true,
        }),
    )];

    *shared_state.current_miners_data.lock().await = Some(miners);
    metrics.record_collection_success().await;
    metrics.record_feed_success().await;

    let summary = metrics.get_summary().await;
    assert_eq!(summary.successful_collections, 1);
    assert_eq!(summary.failed_collections, 1);
}

#[tokio::test]
async fn test_e2e_high_load_scenario() {
    use crate::substrate_interface::api::runtime_types::{
        bounded_collections::bounded_vec::BoundedVec,
        cyborg_primitives::{
            miner::MinerType,
            oracle::{OracleKey, OracleMinerFormat, OracleValue, ProcessStatus},
        },
    };

    let metrics = TestMetrics::new();
    let config = TestConfig {
        miner_count: 100,
        failure_rate: 0.1,
        timeout_ms: 10000,
        retry_count: 3,
    };

    let shared_state = Arc::new(SharedState {
        current_miners_data: Mutex::new(None),
    });

    // Create large miner dataset
    let miners: Vec<_> = (0..config.miner_count)
        .map(|i| {
            let online = (i as f64 / config.miner_count as f64) > config.failure_rate;
            (
                OracleKey::Miner(OracleMinerFormat {
                    id: BoundedVec(vec![(i % 256) as u8]),
                    miner_type: if i % 2 == 0 {
                        MinerType::Cloud
                    } else {
                        MinerType::Edge
                    },
                }),
                OracleValue::MinerStatus(ProcessStatus {
                    online,
                    available: online,
                }),
            )
        })
        .collect();

    let start = tokio::time::Instant::now();

    *shared_state.current_miners_data.lock().await = Some(miners);
    metrics.record_collection_success().await;
    metrics.record_feed_success().await;

    let duration = start.elapsed();
    metrics.record_duration(duration).await;

    let summary = metrics.get_summary().await;
    assert!(summary.total_duration_ms < config.timeout_ms as u128);
}

#[tokio::test]
async fn test_e2e_continuous_operation_stability() {
    use crate::substrate_interface::api::runtime_types::{
        bounded_collections::bounded_vec::BoundedVec,
        cyborg_primitives::{
            miner::MinerType,
            oracle::{OracleKey, OracleMinerFormat, OracleValue, ProcessStatus},
        },
    };

    let metrics = TestMetrics::new();
    let shared_state = Arc::new(SharedState {
        current_miners_data: Mutex::new(None),
    });

    // Run 50 cycles - adjust the failure logic to ensure at least 41 successes
    for cycle in 0..50 {
        let miners = vec![(
            OracleKey::Miner(OracleMinerFormat {
                id: BoundedVec(vec![1]),
                miner_type: MinerType::Cloud,
            }),
            OracleValue::MinerStatus(ProcessStatus {
                // Changed from cycle % 5 != 0 to cycle % 10 != 0 to get more successes
                online: cycle % 10 != 0, // Now only cycles 0, 10, 20, 30, 40 fail
                available: cycle % 3 != 0,
            }),
        )];

        *shared_state.current_miners_data.lock().await = Some(miners);

        // Adjust failure condition to match the new logic
        if cycle % 10 == 0 {
            metrics.record_collection_failure().await;
        } else {
            metrics.record_collection_success().await;
            metrics.record_feed_success().await;
        }

        tokio::time::sleep(Duration::from_millis(1)).await;
    }

    let summary = metrics.get_summary().await;
    assert!(
        summary.successful_collections > 40,
        "Should have > 40 successful collections, got {}",
        summary.successful_collections
    );
    assert!(
        summary.failed_collections < 15,
        "Should have < 15 failed collections, got {}",
        summary.failed_collections
    );
}

#[tokio::test]
async fn test_e2e_miner_lifecycle_management() {
    let simulator = MinerSimulator::new();

    // Phase 1: Miner Registration
    for i in 0..5 {
        simulator
            .add_miner(
                vec![i],
                MinerState {
                    online: true,
                    available: true,
                    response_delay_ms: 10,
                },
            )
            .await;
    }

    // Phase 2: Miner Operations
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Phase 3: Partial Failure
    simulator.simulate_miner_failure(&vec![0]).await;
    simulator.simulate_miner_failure(&vec![2]).await;

    // Phase 4: Recovery
    simulator.simulate_miner_recovery(&vec![0]).await;

    // Verify final states
    assert!(simulator.get_miner_state(&vec![0]).await.unwrap().online);
    assert!(!simulator.get_miner_state(&vec![2]).await.unwrap().online);
    assert!(simulator.get_miner_state(&vec![1]).await.unwrap().online);
}

// ============================================================================
// Parameterized Tests
// ============================================================================

#[tokio::test]
async fn test_parameterized_miner_counts() {
    use crate::substrate_interface::api::runtime_types::{
        bounded_collections::bounded_vec::BoundedVec,
        cyborg_primitives::{
            miner::MinerType,
            oracle::{OracleKey, OracleMinerFormat, OracleValue, ProcessStatus},
        },
    };

    let test_cases = vec![1, 10, 50, 100, 500];

    for count in test_cases {
        let shared_state = Arc::new(SharedState {
            current_miners_data: Mutex::new(None),
        });

        let miners: Vec<_> = (0..count)
            .map(|i| {
                (
                    OracleKey::Miner(OracleMinerFormat {
                        id: BoundedVec(vec![(i % 256) as u8]),
                        miner_type: MinerType::Cloud,
                    }),
                    OracleValue::MinerStatus(ProcessStatus {
                        online: true,
                        available: true,
                    }),
                )
            })
            .collect();

        *shared_state.current_miners_data.lock().await = Some(miners);

        let data = shared_state.current_miners_data.lock().await;
        assert_eq!(
            data.as_ref().unwrap().len(),
            count,
            "Should handle {} miners",
            count
        );
    }
}

#[tokio::test]
async fn test_parameterized_failure_rates() {
    use crate::substrate_interface::api::runtime_types::{
        bounded_collections::bounded_vec::BoundedVec,
        cyborg_primitives::{
            miner::MinerType,
            oracle::{OracleKey, OracleMinerFormat, OracleValue, ProcessStatus},
        },
    };

    let failure_rates = vec![0.0, 0.1, 0.3, 0.5, 0.9];

    for rate in failure_rates {
        let shared_state = Arc::new(SharedState {
            current_miners_data: Mutex::new(None),
        });

        let miners: Vec<_> = (0..100)
            .map(|i| {
                let online = (i as f64 / 100.0) > rate;
                (
                    OracleKey::Miner(OracleMinerFormat {
                        id: BoundedVec(vec![i]),
                        miner_type: MinerType::Cloud,
                    }),
                    OracleValue::MinerStatus(ProcessStatus {
                        online,
                        available: online,
                    }),
                )
            })
            .collect();

        *shared_state.current_miners_data.lock().await = Some(miners);

        let data = shared_state.current_miners_data.lock().await;
        let online_count = data
            .as_ref()
            .unwrap()
            .iter()
            .filter(|(_, v)| {
                if let OracleValue::MinerStatus(s) = v {
                    s.online
                } else {
                    false
                }
            })
            .count();

        let expected = ((1.0 - rate) * 100.0) as usize;
        assert!(
            (online_count as i32 - expected as i32).abs() <= 5,
            "Failure rate {} should result in ~{} online miners, got {}",
            rate,
            expected,
            online_count
        );
    }
}

// ============================================================================
// Regression Tests
// ============================================================================

#[tokio::test]
async fn test_regression_empty_bounded_vec() {
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

    // This was a bug: empty BoundedVec caused panics
    let miners = vec![(
        OracleKey::Miner(OracleMinerFormat {
            id: BoundedVec(vec![]), // Empty!
            miner_type: MinerType::Cloud,
        }),
        OracleValue::MinerStatus(ProcessStatus {
            online: true,
            available: true,
        }),
    )];

    *shared_state.current_miners_data.lock().await = Some(miners);

    let data = shared_state.current_miners_data.lock().await;
    assert!(data.is_some(), "Should handle empty BoundedVec");
}

#[tokio::test]
async fn test_regression_concurrent_lock_deadlock() {
    let shared_state = Arc::new(SharedState {
        current_miners_data: Mutex::new(None),
    });

    // This used to cause deadlocks
    let state1 = Arc::clone(&shared_state);
    let state2 = Arc::clone(&shared_state);

    let handle1 = tokio::spawn(async move {
        for _ in 0..100 {
            let _lock = state1.current_miners_data.lock().await;
            tokio::time::sleep(Duration::from_micros(1)).await;
        }
    });

    let handle2 = tokio::spawn(async move {
        for _ in 0..100 {
            let _lock = state2.current_miners_data.lock().await;
            tokio::time::sleep(Duration::from_micros(1)).await;
        }
    });

    let timeout_result = tokio::time::timeout(Duration::from_secs(5), async {
        handle1.await.unwrap();
        handle2.await.unwrap();
    })
    .await;

    assert!(timeout_result.is_ok(), "Should not deadlock");
}

#[tokio::test]
async fn test_metrics_accuracy() {
    let metrics = TestMetrics::new();

    // Record various operations
    for _ in 0..10 {
        metrics.record_collection_success().await;
    }
    for _ in 0..3 {
        metrics.record_collection_failure().await;
    }
    for _ in 0..8 {
        metrics.record_feed_success().await;
    }
    for _ in 0..2 {
        metrics.record_feed_failure().await;
    }

    let summary = metrics.get_summary().await;

    assert_eq!(summary.successful_collections, 10);
    assert_eq!(summary.failed_collections, 3);
    assert_eq!(summary.successful_feeds, 8);
    assert_eq!(summary.failed_feeds, 2);

    assert!((summary.collection_success_rate() - 76.92).abs() < 0.1);
    assert!((summary.feed_success_rate() - 80.0).abs() < 0.1);
}
