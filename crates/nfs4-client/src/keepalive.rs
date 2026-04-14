use std::sync::Arc;

use tokio::sync::watch;
use tokio::time::{interval, Duration};

use nfs4_types::Compound4args;
use nfs4_types::Compound4res;

use crate::compound::CompoundBuilder;
use crate::error::NfsError;
use crate::session::SessionManager;

/// Runs a background keepalive loop that sends bare SEQUENCE compounds
/// to prevent the NFSv4.1 session from expiring.
///
/// The loop fires at 1/3 of the server's lease interval, giving two
/// full missed ticks before the lease actually expires. Each tick
/// allocates a slot, sends a SEQUENCE-only compound via the caller-
/// supplied closure, and releases the slot. If every slot is busy the
/// tick is silently skipped; if the compound fails the error is
/// ignored so transient network hiccups don't kill the session.
///
/// The loop exits cleanly when `shutdown` receives a new value.
///
/// # Arguments
///
/// * `session`        - shared session manager for slot allocation
/// * `shutdown`       - watch channel; any value change signals exit
/// * `send_compound`  - closure that dispatches a `Compound4args` over
///                      the wire and returns the server's response
pub async fn keepalive_loop<F, Fut>(
    session: Arc<SessionManager>,
    mut shutdown: watch::Receiver<bool>,
    send_compound: F,
) where
    F: Fn(Compound4args) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<Compound4res, NfsError>> + Send,
{
    let lease = session.lease_time() as u64;
    let keepalive_interval = Duration::from_secs(lease / 3);
    let mut timer = interval(keepalive_interval);

    // The first tick from `interval()` fires immediately; consume it
    // so we don't send a keepalive the instant the loop starts.
    timer.tick().await;

    loop {
        tokio::select! {
            _ = timer.tick() => {
                let Some((slot, seq, highest)) = session.try_alloc_slot().await else {
                    // All slots are busy — skip this tick rather than
                    // blocking.  The real traffic that occupied the
                    // slots already renewed the lease.
                    continue;
                };

                let args = CompoundBuilder::new()
                    .tag("keepalive")
                    .sequence(session.session_id().clone(), seq, slot, highest)
                    .build();

                let result = send_compound(args).await;
                session.release_slot(slot).await;

                if let Err(_e) = result {
                    // Transient failures are expected (network blips,
                    // server restarts). Logging would go here once
                    // tracing is wired in; for now we silently retry
                    // on the next tick.
                }
            }
            _ = shutdown.changed() => {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    use nfs4_types::ops::session::*;
    use nfs4_types::{ClientId4, NfsResOp4, NfsStat4, SessionId4};

    #[tokio::test(start_paused = true)]
    async fn keepalive_sends_sequences() {
        let session = Arc::new(SessionManager::new(
            SessionId4([0xBB; 16]),
            ClientId4(999),
            4,
            3, // 3-second lease => keepalive every 1 second
        ));

        let call_count = Arc::new(AtomicU32::new(0));
        let count_clone = call_count.clone();

        let (tx, rx) = watch::channel(false);

        let handle = tokio::spawn(keepalive_loop(session, rx, move |_args| {
            let count = count_clone.clone();
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                Ok(Compound4res {
                    status: NfsStat4::Ok,
                    tag: String::new(),
                    resarray: vec![NfsResOp4::Sequence(Sequence4res {
                        status: NfsStat4::Ok,
                        resok: Some(Sequence4resok {
                            sr_sessionid: SessionId4([0xBB; 16]),
                            sr_sequenceid: 1,
                            sr_slotid: 0,
                            sr_highest_slotid: 3,
                            sr_target_highest_slotid: 3,
                            sr_status_flags: 0,
                        }),
                    })],
                })
            }
        }));

        // Advance past two keepalive intervals (1 s each), yielding
        // between steps so the spawned task can process each tick.
        for _ in 0..3 {
            tokio::time::advance(Duration::from_secs(1)).await;
            tokio::task::yield_now().await;
        }

        tx.send(true).unwrap();
        handle.await.unwrap();

        let count = call_count.load(Ordering::SeqCst);
        assert!(count >= 2, "expected at least 2 keepalives, got {}", count);
    }

    #[tokio::test(start_paused = true)]
    async fn keepalive_stops_on_shutdown() {
        let session = Arc::new(SessionManager::new(
            SessionId4([0xCC; 16]),
            ClientId4(888),
            4,
            30, // 30-second lease => keepalive every 10 seconds
        ));

        let (tx, rx) = watch::channel(false);

        let handle = tokio::spawn(keepalive_loop(session, rx, |_args| async {
            Ok(Compound4res {
                status: NfsStat4::Ok,
                tag: String::new(),
                resarray: vec![],
            })
        }));

        // Signal shutdown immediately.
        tx.send(true).unwrap();

        // The task should exit well before the first keepalive tick.
        tokio::time::timeout(Duration::from_secs(2), handle)
            .await
            .expect("keepalive should stop within 2 seconds")
            .unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn keepalive_tolerates_send_failures() {
        let session = Arc::new(SessionManager::new(
            SessionId4([0xDD; 16]),
            ClientId4(777),
            4,
            3,
        ));

        let call_count = Arc::new(AtomicU32::new(0));
        let count_clone = call_count.clone();

        let (tx, rx) = watch::channel(false);

        let handle = tokio::spawn(keepalive_loop(session, rx, move |_args| {
            let count = count_clone.clone();
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                Err(NfsError::Nfs(NfsStat4::Delay))
            }
        }));

        // Advance past two keepalive intervals, yielding between
        // steps so the spawned task processes each tick.
        for _ in 0..3 {
            tokio::time::advance(Duration::from_secs(1)).await;
            tokio::task::yield_now().await;
        }

        tx.send(true).unwrap();
        handle.await.unwrap();

        let count = call_count.load(Ordering::SeqCst);
        assert!(
            count >= 2,
            "loop should keep running after failures, got {} calls",
            count
        );
    }
}
