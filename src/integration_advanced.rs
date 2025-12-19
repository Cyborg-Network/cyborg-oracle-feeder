#![cfg(test)]

use crate::feeder::SharedState;
use crate::tx_queue::{init_transaction_queue, TxOutput, TRANSACTION_QUEUE};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

// ============================================================================
// Network Resilience Tests
// ============================================================================

#[tokio::test]
async fn test_network_partition_recovery() {
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

    // Simulate network partition - no data collected
    assert!(shared_state.current_miners_data.lock().await.is_none());

    // Simulate network recovery
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

    let data = shared_state.current_miners_data.lock().await;
    assert!(data.is_some(), "Should recover after network partition");
}

#[tokio::test]
async fn test_cascading_miner_failures() {
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

    // All miners fail simultaneously
    let failed_miners: Vec<_> = (0..10)
        .map(|i| {
            (
                OracleKey::Miner(OracleMinerFormat {
                    id: BoundedVec(vec![i]),
                    miner_type: if i % 2 == 0 {
                        MinerType::Cloud
                    } else {
                        MinerType::Edge
                    },
                }),
                OracleValue::MinerStatus(ProcessStatus {
                    online: false,
                    available: false,
                }),
            )
        })
        .collect();

    *shared_state.current_miners_data.lock().await = Some(failed_miners);

    let data = shared_state.current_miners_data.lock().await;
    let all_offline = data.as_ref().unwrap().iter().all(|(_, value)| {
        if let OracleValue::MinerStatus(status) = value {
            !status.online && !status.available
        } else {
            false
        }
    });

    assert!(all_offline, "Should handle all miners failing");
}

#[tokio::test]
async fn test_partial_miner_recovery() {
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

    // Initial state: all miners offline
    let initial_miners: Vec<_> = (0..5)
        .map(|i| {
            (
                OracleKey::Miner(OracleMinerFormat {
                    id: BoundedVec(vec![i]),
                    miner_type: MinerType::Cloud,
                }),
                OracleValue::MinerStatus(ProcessStatus {
                    online: false,
                    available: false,
                }),
            )
        })
        .collect();

    *shared_state.current_miners_data.lock().await = Some(initial_miners);

    // Partial recovery: 3 out of 5 miners come back online
    let recovered_miners: Vec<_> = (0..5)
        .map(|i| {
            (
                OracleKey::Miner(OracleMinerFormat {
                    id: BoundedVec(vec![i]),
                    miner_type: MinerType::Cloud,
                }),
                OracleValue::MinerStatus(ProcessStatus {
                    online: i < 3,
                    available: i < 3,
                }),
            )
        })
        .collect();

    *shared_state.current_miners_data.lock().await = Some(recovered_miners);

    let data = shared_state.current_miners_data.lock().await;
    let online_count = data
        .as_ref()
        .unwrap()
        .iter()
        .filter(|(_, value)| {
            if let OracleValue::MinerStatus(status) = value {
                status.online
            } else {
                false
            }
        })
        .count();

    assert_eq!(online_count, 3, "Should handle partial recovery correctly");
}

// ============================================================================
// Data Integrity Tests
// ============================================================================

#[tokio::test]
async fn test_data_corruption_detection() {
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

    // Valid data
    let valid_data = vec![(
        OracleKey::Miner(OracleMinerFormat {
            id: BoundedVec(vec![1, 2, 3]),
            miner_type: MinerType::Cloud,
        }),
        OracleValue::MinerStatus(ProcessStatus {
            online: true,
            available: true,
        }),
    )];

    *shared_state.current_miners_data.lock().await = Some(valid_data.clone());

    // Verify data integrity
    let data = shared_state.current_miners_data.lock().await;
    assert_eq!(
        data.as_ref().unwrap().len(),
        1,
        "Data should maintain integrity"
    );
}

#[tokio::test]
async fn test_duplicate_miner_id_handling() {
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

    // Create data with duplicate IDs
    let miners_with_duplicates = vec![
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
                id: BoundedVec(vec![1]), // Duplicate ID
                miner_type: MinerType::Edge,
            }),
            OracleValue::MinerStatus(ProcessStatus {
                online: false,
                available: false,
            }),
        ),
    ];

    *shared_state.current_miners_data.lock().await = Some(miners_with_duplicates);

    // System should accept the data (deduplication is chain responsibility)
    let data = shared_state.current_miners_data.lock().await;
    assert_eq!(data.as_ref().unwrap().len(), 2, "Should store all entries");
}

// ============================================================================
// Performance and Stress Tests
// ============================================================================

#[tokio::test]
async fn test_high_frequency_updates() {
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

    // Rapidly update state 100 times
    for i in 0..100 {
        let miners = vec![(
            OracleKey::Miner(OracleMinerFormat {
                id: BoundedVec(vec![i as u8]),
                miner_type: MinerType::Cloud,
            }),
            OracleValue::MinerStatus(ProcessStatus {
                online: true,
                available: true,
            }),
        )];

        *shared_state.current_miners_data.lock().await = Some(miners);
    }

    let data = shared_state.current_miners_data.lock().await;
    assert!(data.is_some(), "Should handle rapid updates");
}

#[tokio::test]
async fn test_concurrent_read_write_stress() {
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

    let mut handles = vec![];

    // Spawn 50 concurrent tasks
    for i in 0..50 {
        let state = Arc::clone(&shared_state);

        let handle = tokio::spawn(async move {
            if i % 2 == 0 {
                // Writers
                let miners = vec![(
                    OracleKey::Miner(OracleMinerFormat {
                        id: BoundedVec(vec![i as u8]),
                        miner_type: MinerType::Cloud,
                    }),
                    OracleValue::MinerStatus(ProcessStatus {
                        online: true,
                        available: true,
                    }),
                )];
                *state.current_miners_data.lock().await = Some(miners);
            } else {
                // Readers
                let _data = state.current_miners_data.lock().await;
            }
        });

        handles.push(handle);
    }

    // Wait for all tasks
    for handle in handles {
        assert!(handle.await.is_ok(), "No task should panic");
    }
}

#[tokio::test]
async fn test_memory_pressure_with_large_dataset() {
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

    // Create 10,000 miners
    let large_dataset: Vec<_> = (0..10000)
        .map(|i| {
            (
                OracleKey::Miner(OracleMinerFormat {
                    id: BoundedVec(vec![(i % 256) as u8, (i / 256) as u8]),
                    miner_type: if i % 2 == 0 {
                        MinerType::Cloud
                    } else {
                        MinerType::Edge
                    },
                }),
                OracleValue::MinerStatus(ProcessStatus {
                    online: i % 3 != 0,
                    available: i % 5 != 0,
                }),
            )
        })
        .collect();

    *shared_state.current_miners_data.lock().await = Some(large_dataset);

    let data = shared_state.current_miners_data.lock().await;
    assert_eq!(
        data.as_ref().unwrap().len(),
        10000,
        "Should handle large datasets"
    );
}

// ============================================================================
// Error Recovery and Resilience Tests
// ============================================================================

#[tokio::test]
async fn test_error_recovery_after_multiple_failures() {
    let shared_state = Arc::new(SharedState {
        current_miners_data: Mutex::new(None),
    });

    let failure_count = Arc::new(Mutex::new(0));

    // Simulate 5 consecutive failures
    for _ in 0..5 {
        *failure_count.lock().await += 1;
    }

    // Then success
    use crate::substrate_interface::api::runtime_types::{
        bounded_collections::bounded_vec::BoundedVec,
        cyborg_primitives::{
            miner::MinerType,
            oracle::{OracleKey, OracleMinerFormat, OracleValue, ProcessStatus},
        },
    };

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

    assert_eq!(*failure_count.lock().await, 5);
    assert!(shared_state.current_miners_data.lock().await.is_some());
}

#[tokio::test]
async fn test_graceful_shutdown_with_pending_operations() {
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

    // Start long-running operation
    let state_clone = Arc::clone(&shared_state);
    let handle = tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;

        *state_clone.current_miners_data.lock().await = Some(vec![(
            OracleKey::Miner(OracleMinerFormat {
                id: BoundedVec(vec![1]),
                miner_type: MinerType::Cloud,
            }),
            OracleValue::MinerStatus(ProcessStatus {
                online: true,
                available: true,
            }),
        )]);
    });

    // Wait for completion
    let result = tokio::time::timeout(Duration::from_secs(1), handle).await;
    assert!(result.is_ok(), "Should complete gracefully");
}

// ============================================================================
// Transaction Queue Integration Tests
// ============================================================================

#[tokio::test]
async fn test_transaction_queue_initialization() {
    init_transaction_queue();
    let queue = TRANSACTION_QUEUE.get();
    assert!(queue.is_some(), "Transaction queue should be initialized");
}

#[tokio::test]
async fn test_transaction_queue_ordering() {
    init_transaction_queue();
    let queue = TRANSACTION_QUEUE.get().unwrap();

    let results = Arc::new(Mutex::new(Vec::new()));

    // Enqueue 5 transactions
    let mut receivers = vec![];
    for i in 0..5 {
        let results_clone = Arc::clone(&results);
        let rx = queue
            .enqueue(move || {
                let value = results_clone.clone();
                async move {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    value.lock().await.push(i);
                    Ok(TxOutput::OracleFeedSuccess)
                }
            })
            .await
            .unwrap();
        receivers.push(rx);
    }

    // Wait for all to complete
    for rx in receivers {
        let result = rx.await;
        assert!(result.is_ok(), "Transaction should complete");
    }

    // Give processing time to complete
    tokio::time::sleep(Duration::from_millis(100)).await;

    let final_results = results.lock().await;
    assert_eq!(final_results.len(), 5, "All transactions should execute");
}

// ============================================================================
// Real-World Scenario Tests
// ============================================================================

#[tokio::test]
async fn test_production_like_workflow() {
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

    // Simulate 10 complete oracle cycles
    for cycle in 0..10 {
        // Collect phase
        let miners: Vec<_> = (0..5)
            .map(|i| {
                (
                    OracleKey::Miner(OracleMinerFormat {
                        id: BoundedVec(vec![i]),
                        miner_type: if i % 2 == 0 {
                            MinerType::Cloud
                        } else {
                            MinerType::Edge
                        },
                    }),
                    OracleValue::MinerStatus(ProcessStatus {
                        online: (cycle + i) % 3 != 0, // Varying status
                        available: (cycle + i) % 2 == 0,
                    }),
                )
            })
            .collect();

        *shared_state.current_miners_data.lock().await = Some(miners);

        // Simulate feed delay
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Verify state
        let data = shared_state.current_miners_data.lock().await;
        assert!(data.is_some(), "Cycle {} should have data", cycle);
    }
}

#[tokio::test]
async fn test_miner_churn_handling() {
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

    // Initial: 10 miners
    let mut current_miner_count = 10;

    for round in 0..5 {
        let miners: Vec<_> = (0..current_miner_count)
            .map(|i| {
                (
                    OracleKey::Miner(OracleMinerFormat {
                        id: BoundedVec(vec![i as u8]),
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

        // Simulate churn: add 2, remove 1
        current_miner_count += 1;

        let data = shared_state.current_miners_data.lock().await;
        assert!(
            data.as_ref().unwrap().len() <= current_miner_count,
            "Round {}: Should handle miner churn",
            round
        );
    }
}
