use std::{
    cell::RefCell,
    time::{Duration, Instant},
};
use tauri::ipc::Channel;

#[derive(Clone, serde::Serialize)]
pub struct TransferProgress {
    pub phase: &'static str,
    pub completed_bytes: u64,
    pub total_bytes: Option<u64>,
}

struct Reporter {
    channel: Channel<TransferProgress>,
    value: TransferProgress,
    last_sent: Instant,
}

thread_local! {
    static CURRENT: RefCell<Option<Reporter>> = const { RefCell::new(None) };
}

// Transfer commands are synchronous function bodies dispatched by Tauri. Keep each
// command's channel scoped to its worker; no process-wide progress or cross-talk.
pub fn run<T>(channel: Option<Channel<TransferProgress>>, work: impl FnOnce() -> T) -> T {
    struct Restore(Option<Reporter>);
    impl Drop for Restore {
        fn drop(&mut self) {
            CURRENT.with(|current| {
                current.replace(self.0.take());
            });
        }
    }
    let previous = CURRENT.with(|current| {
        current.replace(channel.map(|channel| Reporter {
            channel,
            value: TransferProgress {
                phase: "preparing",
                completed_bytes: 0,
                total_bytes: None,
            },
            last_sent: Instant::now(),
        }))
    });
    let _restore = Restore(previous);
    stage("preparing", None);
    work()
}

pub fn stage(phase: &'static str, total_bytes: Option<u64>) {
    CURRENT.with(|current| {
        if let Some(reporter) = current.borrow_mut().as_mut() {
            reporter.value = TransferProgress {
                phase,
                completed_bytes: 0,
                total_bytes,
            };
            let _ = reporter.channel.send(reporter.value.clone());
            reporter.last_sent = Instant::now();
        }
    });
}

pub fn advance(bytes: u64) {
    CURRENT.with(|current| {
        if let Some(reporter) = current.borrow_mut().as_mut() {
            reporter.value.completed_bytes = reporter.value.completed_bytes.saturating_add(bytes);
            if reporter.last_sent.elapsed() >= Duration::from_millis(120) {
                let _ = reporter.channel.send(reporter.value.clone());
                reporter.last_sent = Instant::now();
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn progress_is_throttled_scoped_and_does_not_change_work_errors() {
        let events = Arc::new(Mutex::new(Vec::<serde_json::Value>::new()));
        let captured = events.clone();
        let channel = Channel::new(move |body| {
            if let tauri::ipc::InvokeResponseBody::Json(json) = body {
                captured
                    .lock()
                    .unwrap()
                    .push(serde_json::from_str(&json).unwrap());
            }
            Ok(())
        });
        let result: Result<(), &str> = run(Some(channel), || {
            stage("import", Some(2000));
            CURRENT.with(|current| {
                current.borrow_mut().as_mut().unwrap().last_sent =
                    Instant::now() + Duration::from_secs(60)
            });
            for _ in 0..1000 {
                advance(1);
            }
            assert_eq!(events.lock().unwrap().len(), 2);
            CURRENT.with(|current| {
                current.borrow_mut().as_mut().unwrap().last_sent =
                    Instant::now() - Duration::from_secs(1)
            });
            advance(1);
            assert_eq!(events.lock().unwrap()[2]["completed_bytes"], 1001);
            run(None, || {
                stage("inner", None);
                advance(99);
            });
            CURRENT.with(|current| {
                assert_eq!(
                    current.borrow().as_ref().unwrap().value.completed_bytes,
                    1001
                )
            });
            stage("commit", None);
            Err("original failure")
        });
        assert_eq!(result, Err("original failure"));
        let count = events.lock().unwrap().len();
        stage("late", None);
        advance(50);
        assert_eq!(events.lock().unwrap().len(), count);
        CURRENT.with(|current| assert!(current.borrow().is_none()));
    }
}
