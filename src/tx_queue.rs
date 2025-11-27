#![allow(dead_code)]
use crate::error::Result;
use once_cell::sync::OnceCell;
use std::{
    collections::VecDeque,
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::sync::{oneshot, Mutex};

const MAX_RETRIES: u32 = 500;

/// The type of an async transaction executor closure: no args, returns a Future Result
pub type TxExecutor =
    Box<dyn Fn() -> Pin<Box<dyn Future<Output = Result<TxOutput>> + Send>> + Send + Sync>;

#[derive(Debug)]
pub enum TxOutput {
    OracleFeedSuccess,
    // ProofVerificationSuccess,
}

pub struct Transaction {
    pub executor: TxExecutor,
    pub responder: Option<oneshot::Sender<Result<TxOutput>>>,
    pub retry_count: u32,
}

impl Transaction {
    pub fn new(executor: TxExecutor, responder: Option<oneshot::Sender<Result<TxOutput>>>) -> Self {
        Self {
            executor,
            retry_count: 0,
            responder,
        }
    }

    pub async fn execute(&self) -> Result<TxOutput> {
        (self.executor)().await
    }

    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
    }

    pub fn retry_count(&self) -> u32 {
        self.retry_count
    }
}

pub struct TransactionQueue {
    pub inner: Arc<Mutex<VecDeque<Transaction>>>,
    pub processing: Arc<AtomicBool>,
}

pub static TRANSACTION_QUEUE: OnceCell<TransactionQueue> = OnceCell::new();

impl TransactionQueue {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(VecDeque::new())),
            processing: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn enqueue<F, Fut>(&self, executor: F) -> Result<oneshot::Receiver<Result<TxOutput>>>
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<TxOutput>> + Send + 'static,
    {
        let (tx, rx) = oneshot::channel();

        let tx = Transaction {
            executor: Box::new(move || Box::pin(executor())),
            responder: Some(tx),
            retry_count: 0,
        };

        self.inner.lock().await.push_back(tx);
        self.start_processing();

        Ok(rx)
    }

    pub fn start_processing(&self) {
        if self.processing.swap(true, Ordering::SeqCst) {
            // Already processing
            return;
        }

        let inner = Arc::clone(&self.inner);
        let processing_flag = Arc::clone(&self.processing);

        tokio::spawn(async move {
            loop {
                let tx_opt = {
                    let mut queue = inner.lock().await;
                    log::debug!("Oracle feeder transaction queue size: {}", queue.len());
                    queue.pop_front()
                };

                match tx_opt {
                    Some(mut tx) => match tx.execute().await {
                        Ok(result) => {
                            log::info!("Oracle feeder transaction succeeded: {result:?}");
                            if let Some(responder) = tx.responder.take() {
                                let _ = responder.send(Ok(result));
                            }
                        }
                        Err(e) if tx.retry_count < MAX_RETRIES => {
                            log::warn!("Oracle feeder transaction failed: {}", e);
                            tx.increment_retry();

                            let delay_ms = 1000 * 2u64.pow(tx.retry_count().min(10));
                            log::debug!("Retrying after {} ms", delay_ms);
                            tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;

                            let mut queue = inner.lock().await;
                            queue.push_front(tx);
                        }
                        Err(e) => {
                            log::error!("Oracle feeder transaction failed after retries: {}", e);
                            if let Some(responder) = tx.responder.take() {
                                let _ = responder.send(Err(e));
                            }
                        }
                    },
                    None => {
                        processing_flag.store(false, Ordering::SeqCst);
                        log::debug!("Oracle feeder transaction queue is empty");
                        break;
                    }
                }
            }
        });
    }
}

/// Initialize the transaction queue
pub fn init_transaction_queue() {
    let _ = TRANSACTION_QUEUE.set(TransactionQueue::new());
}
