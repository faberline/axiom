use super::async_rt::{alloc_task_id, mb_coroutine_step, MbTask, COROUTINES, TASKS};
use super::rc::{MbObject, ObjData};
/// Tokio multi-threaded executor for Mamba async tasks (R6).
///
/// Delegates to the process-wide shared runtime in `mamba-registry` so
/// the interpreter and every native module (mambalibs_http::client, pgkit, mcp, etc.)
/// dispatch onto a single tokio runtime instead of each owning one.
use super::value::MbValue;

/// Handle to the shared mamba tokio runtime (hosted in mamba-registry).
fn runtime() -> tokio::runtime::Handle {
    mamba_registry::runtime::handle()
}

/// Spawn a coroutine as a Tokio task for parallel execution.
/// Returns a task handle (same as mb_create_task).
pub fn mb_tokio_spawn(coro: MbValue) -> MbValue {
    let coro_id = coro.as_int().unwrap_or(0) as u64;
    let task = MbTask {
        name: format!("tokio-task-{coro_id}"),
        coroutine_id: coro_id,
        done: false,
        cancelled: false,
        result: MbValue::none(),
    };
    let task_id = alloc_task_id();
    TASKS.write().unwrap().insert(task_id, task);

    let rt = runtime();
    rt.spawn(async move {
        // Drive the coroutine to completion
        let coro_handle = MbValue::from_int(coro_id as i64);
        let max_steps = 100_000;
        for _ in 0..max_steps {
            super::gc::gc_safepoint();
            let exhausted = COROUTINES
                .read()
                .unwrap()
                .get(&coro_id)
                .map(|c| c.exhausted)
                .unwrap_or(true);
            if exhausted {
                break;
            }
            mb_coroutine_step(coro_handle);
            // Yield to Tokio scheduler between steps
            tokio::task::yield_now().await;
        }

        // Propagate result to task
        let result = COROUTINES
            .read()
            .unwrap()
            .get(&coro_id)
            .and_then(|c| c.result)
            .unwrap_or(MbValue::none());
        if let Some(task) = TASKS.write().unwrap().get_mut(&task_id) {
            task.done = true;
            task.cancelled = false;
            task.result = result;
        }
    });

    MbValue::from_int(task_id as i64)
}

/// Run multiple coroutines in parallel on the Tokio thread pool.
/// Blocks until all complete and returns a list of results.
pub fn mb_tokio_gather(coros: MbValue) -> MbValue {
    let coro_ids: Vec<u64> = if let Some(ptr) = coros.as_ptr() {
        unsafe {
            if let ObjData::List(ref lock) = (*ptr).data {
                lock.read()
                    .unwrap()
                    .iter()
                    .filter_map(|c| c.as_int().map(|i| i as u64))
                    .collect()
            } else {
                return MbValue::none();
            }
        }
    } else {
        return MbValue::none();
    };

    if coro_ids.is_empty() {
        return MbValue::from_ptr(MbObject::new_list(vec![]));
    }

    let rt = runtime();
    let mut handles = Vec::with_capacity(coro_ids.len());
    let mut task_ids = Vec::with_capacity(coro_ids.len());

    for &coro_id in &coro_ids {
        let task = MbTask {
            name: format!("tokio-gather-{coro_id}"),
            coroutine_id: coro_id,
            done: false,
            cancelled: false,
            result: MbValue::none(),
        };
        let task_id = alloc_task_id();
        TASKS.write().unwrap().insert(task_id, task);
        task_ids.push(task_id);

        let handle = rt.spawn(async move {
            let coro_handle = MbValue::from_int(coro_id as i64);
            let max_steps = 100_000;
            for _ in 0..max_steps {
                super::gc::gc_safepoint();
                let exhausted = COROUTINES
                    .read()
                    .unwrap()
                    .get(&coro_id)
                    .map(|c| c.exhausted)
                    .unwrap_or(true);
                if exhausted {
                    break;
                }
                mb_coroutine_step(coro_handle);
                tokio::task::yield_now().await;
            }

            COROUTINES
                .read()
                .unwrap()
                .get(&coro_id)
                .and_then(|c| c.result)
                .unwrap_or(MbValue::none())
        });
        handles.push(handle);
    }

    // Block current thread until all tasks complete
    let results: Vec<MbValue> = rt.block_on(async {
        let mut results = Vec::with_capacity(handles.len());
        for (i, handle) in handles.into_iter().enumerate() {
            let result = handle.await.unwrap_or(MbValue::none());
            // Update task state
            if let Some(task) = TASKS.write().unwrap().get_mut(&task_ids[i]) {
                task.done = true;
                task.cancelled = false;
                task.result = result;
            }
            results.push(result);
        }
        results
    });

    MbValue::from_ptr(MbObject::new_list(results))
}

/// Shutdown the Tokio runtime (call at interpreter exit).
pub fn mb_tokio_shutdown() {
    // OnceLock doesn't support take(), so runtime lives for process lifetime.
    // Tokio handles cleanup on process exit.
}

/// Test helper: poll a task id until `done` or the deadline elapses. A fixed
/// sleep is flaky under full-suite parallel load.
#[cfg(test)]
pub(crate) fn wait_task_done(task_id: u64, timeout: std::time::Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        let done = TASKS
            .read()
            .unwrap()
            .get(&task_id)
            .map(|t| t.done)
            .unwrap_or(false);
        if done {
            return true;
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[cfg(test)]
mod tests {
    use super::super::async_rt::{mb_coroutine_complete, mb_coroutine_new, ASYNC_STATE_TEST_LOCK};
    use super::super::gc::{gc_disable, gc_enable};
    use super::*;

    fn reset_async_for_test() {
        COROUTINES.write().unwrap().clear();
        TASKS.write().unwrap().clear();
        // Prevent concurrent GC from freeing test MbObjects
        gc_disable();
    }

    #[test]
    fn test_runtime_initializes() {
        // Just verify the handle is reachable and dispatches work
        let rt = runtime();
        rt.block_on(async { 42 });
    }

    #[test]
    fn test_tokio_spawn_completed_coro() {
        let _lock = ASYNC_STATE_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        reset_async_for_test();
        let name = MbValue::from_ptr(MbObject::new_str("tokio_test".to_string()));
        let locals = MbValue::from_ptr(MbObject::new_list(vec![]));
        let coro = mb_coroutine_new(name, locals);
        mb_coroutine_complete(coro, MbValue::from_int(42));

        let task = mb_tokio_spawn(coro);
        assert!(task.is_int());

        let task_id = task.as_int().unwrap() as u64;
        let done = wait_task_done(task_id, std::time::Duration::from_secs(60));
        assert!(done, "Tokio task should complete for exhausted coroutine");
        gc_enable();
    }

    #[test]
    fn test_tokio_gather_empty() {
        let _lock = ASYNC_STATE_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        reset_async_for_test();
        let coros = MbValue::from_ptr(MbObject::new_list(vec![]));
        let result = mb_tokio_gather(coros);
        assert!(
            result.as_ptr().is_some(),
            "gather of empty list should return list"
        );
        gc_enable();
    }

    #[test]
    fn test_tokio_gather_completed_coros() {
        let _lock = ASYNC_STATE_TEST_LOCK
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        reset_async_for_test();
        let n1 = MbValue::from_ptr(MbObject::new_str("g1".to_string()));
        let l1 = MbValue::from_ptr(MbObject::new_list(vec![]));
        let c1 = mb_coroutine_new(n1, l1);
        mb_coroutine_complete(c1, MbValue::from_int(10));

        let n2 = MbValue::from_ptr(MbObject::new_str("g2".to_string()));
        let l2 = MbValue::from_ptr(MbObject::new_list(vec![]));
        let c2 = mb_coroutine_new(n2, l2);
        mb_coroutine_complete(c2, MbValue::from_int(20));

        let coros = MbValue::from_ptr(MbObject::new_list(vec![c1, c2]));
        let result = mb_tokio_gather(coros);
        assert!(result.as_ptr().is_some(), "gather should return a list");
        gc_enable();
    }
}
