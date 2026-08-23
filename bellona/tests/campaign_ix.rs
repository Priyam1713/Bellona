//! Campaign IX: exactly-once delegation semantics.

use async_trait::async_trait;
use foedus::a2a::{A2aService, IdempotencyStore, TaskExecutor, TaskRequest, TaskResponse};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

// ---------- stores ----------

#[derive(Default, Clone)]
struct MemStore(Arc<Mutex<std::collections::HashMap<String, String>>>);

#[async_trait]
impl IdempotencyStore for MemStore {
    async fn claim(&self, key: &str) -> Result<Option<String>, String> {
        Ok(self.0.lock().unwrap().get(key).cloned())
    }
    async fn complete(&self, key: &str, resp: &str) -> Result<(), String> {
        self.0.lock().unwrap().insert(key.into(), resp.into());
        Ok(())
    }
}

// ---------- executors ----------

struct CountingExecutor {
    executions: Arc<AtomicU32>,
}

#[async_trait]
impl TaskExecutor for CountingExecutor {
    async fn execute(&self, req: &TaskRequest) -> Result<TaskResponse, String> {
        self.executions.fetch_add(1, Ordering::SeqCst);
        Ok(TaskResponse::Completed {
            artifacts: serde_json::json!({ "echo": req.instruction }),
        })
    }
}

struct FlakyExecutor {
    attempts: Arc<AtomicU32>,
    fails_first_n: usize,
}

#[async_trait]
impl TaskExecutor for FlakyExecutor {
    async fn execute(&self, _req: &TaskRequest) -> Result<TaskResponse, String> {
        let n = self.attempts.fetch_add(1, Ordering::SeqCst);
        if (n as usize) < self.fails_first_n {
            Err("transient failure".into())
        } else {
            Ok(TaskResponse::Completed {
                artifacts: serde_json::json!({ "attempt": n }),
            })
        }
    }
}

fn mk_req(key: &str) -> TaskRequest {
    TaskRequest {
        task_id: "t1".into(),
        idempotency_key: key.into(),
        instruction: "scout the flanks".into(),
        context: serde_json::Value::Null,
    }
}

#[tokio::test]
async fn replayed_idempotency_key_does_not_reexecute() {
    let executions = Arc::new(AtomicU32::new(0));
    let svc = A2aService {
        store: MemStore::default(),
        executor: CountingExecutor {
            executions: executions.clone(),
        },
    };

    let r1 = svc.handle(mk_req("k-1")).await.unwrap();
    let r2 = svc.handle(mk_req("k-1")).await.unwrap(); // replay
    let _r3 = svc.handle(mk_req("k-2")).await.unwrap(); // genuinely new

    assert_eq!(
        executions.load(Ordering::SeqCst),
        2,
        "replay must not re-execute"
    );
    match (r1, r2) {
        (TaskResponse::Completed { artifacts: a }, TaskResponse::Completed { artifacts: b }) => {
            assert_eq!(a, b, "replayed response identical");
            assert_eq!(a["echo"], "scout the flanks");
        }
        _ => panic!("expected completions"),
    }
}

#[tokio::test]
async fn executor_failure_is_not_cached_so_retry_can_succeed() {
    let attempts = Arc::new(AtomicU32::new(0));
    let svc = A2aService {
        store: MemStore::default(),
        executor: FlakyExecutor {
            attempts: attempts.clone(),
            fails_first_n: 1,
        },
    };

    let req = mk_req("same-key");
    assert!(
        svc.handle(req.clone()).await.is_err(),
        "first attempt fails"
    );

    // The failed attempt must NOT have poisoned the slot.
    let ok = svc.handle(req).await.unwrap();
    match ok {
        TaskResponse::Completed { artifacts } => {
            assert_eq!(attempts.load(Ordering::SeqCst), 2);
            assert_eq!(artifacts["attempt"], 1);
        }
        other => panic!("retry should complete: {other:?}"),
    }
}
