use std::{
    fmt::Debug,
    future::Future,
};

use tokio::sync::mpsc::{
    Receiver,
    Sender,
};

use crate::{
    message::{
        Request,
        Response,
    },
    Pid,
};

// NOTE(jdb): `Clone` bound _seems_ unnecessary. I'm currently including this
// in order to bypass move issues when matching `self.state` and moving the
// `State::Running { task }` to `Workable::process`. This needs to be designed
// better later on.
pub trait Task: Send + Sync + Clone {}

/// A long-lived Worker/Actor that is sent tasks to execute and emit back events
/// to be handled by the supervisor.
pub trait Workable: Debug + Send + Sync + Sized {
    //  In the future, we may consider a way to reuse the existing worker
    //  by restarting it with the same task with a clean context.
    type Task: Task + Debug;
    type Output: Send + Debug;
    type Error: Send + Debug;

    fn process(task: Self::Task) -> impl Future<Output = Result<Self::Output, Self::Error>> + Send;
}

#[derive(Debug)]
pub struct Worker<W: Workable> {
    id: Pid,
    tx: Sender<(Pid, Response<W>)>,
    rx: Receiver<Request<W>>,
    state: State<W>,
}

impl<W: Workable> Drop for Worker<W> {
    fn drop(&mut self) {
        // NOTE(jdb): Perform clean shutdown as recommended by tokio
        //
        // https://docs.rs/tokio/latest/tokio/sync/mpsc/index.html#clean-shutdown
        self.rx.close();
        while self.rx.try_recv().is_ok() {}
    }
}

impl<W: Workable> Worker<W> {
    pub fn new(id: Pid, tx: Sender<(Pid, Response<W>)>, rx: Receiver<Request<W>>) -> Self {
        Self {
            id,
            tx,
            rx,
            state: State::Idle,
        }
    }

    #[tracing::instrument(skip(self), fields(worker_id = self.id.to_string()))]
    pub async fn run(mut self) {
        loop {
            tokio::select! {
                Some(event) = self.rx.recv() => {
                    tracing::trace!("received event {event:?}");

                    match self.state.next(event) {
                        Ok(state) => {

                        // We've successfully transitioned our state and
                        // should handle the transition accordingly. I don't
                        // quite like how the state handling is split up,
                        // it's really ugly. But we make it work first, then
                        // we make it fast/pretty.
                        self.state = state;

                        tracing::trace!("transitioned to state {state:?}", state = self.state);
                        match &self.state {
                            State::Running { task } => {
                                // FIXME(jdb): Right now this blocks the state
                                // transition until the task is wrong which is
                                // not right. The issue with this is that we
                                // are no longer able to listen to messages while
                                // the task is running (important for task
                                // cancellation).
                                let result = W::process(task.clone()).await;


                                let message = match result {
                                    Ok(result) => Response::Complete(Ok(result)),
                                    Err(e) => Response::Complete(Err((e, task.clone())))
                                };

                                self.state = State::Idle;
                                let _ = self.tx.send((self.id, message)).await;

                            }
                            State::Idle => {
                                // Do nothing, wait for a new task
                            }
                            State::Error(err) => {
                                // Something bad happened
                                tracing::error!("error during execution: {err:?}");
                            }
                            State::Stop => {
                                tracing::debug!("received shutdown signal from supervisor");
                                let _ = self.tx.send((self.id, Response::ShutdownAck)).await;
                                return;
                            }
                        }

                        },
                        Err(e) => {
                            tracing::error!("state transition error: {e:?}");
                            return;
                        }
                    }
                }
                else => {
                    tracing::error!("worker channel closed: shutting down");
                    break;
                }
            }
        }
    }
}

#[allow(dead_code)]
#[derive(Debug)]
enum State<W: Workable> {
    Idle,
    Running { task: W::Task },
    Error(W::Error),
    Stop,
}

impl<W: Workable> State<W> {
    // The following is the initial state machine for workers.
    //
    //                           complete/success
    //                           cancel
    //              ┌───────────────────────────────┐
    //              │                               │
    //         ┌────▼─────┐                   ┌─────┴─────┐
    //       ┌─┤          │      task         │           │
    // cancel│ │   IDLE   ├───────────────────►  RUNNING  │
    //       └─►          │                   │           │
    //         └────┬─────┘                   └───┬─┬─────┘
    //              │            shutdown         │ │
    //      shutdown│ ┌───────────────────────────┘ │complete/fail
    //              │ │                             │
    //         ┌────▼─▼───┐                   ┌─────▼─────┐
    //         │          │      shutdown     │           │
    //         │   STOP   ◄───────────────────┤   ERROR   │
    //         │          │                   │           │
    //         └──────────┘                   └───────────┘
    //
    //  Note that retrying should not be handled by the worker. Instead it
    //  is the job of the Supervisor to determine if we should shut down
    //  the worker, and spawn a new one with the same task. This is done to
    //  ensure that we are able to capture error state before terminating a
    //  worker.
    fn next(&self, event: Request<W>) -> Result<State<W>, String> {
        match (self, &event) {
            (State::Idle, Request::Task(t)) => Ok(State::Running { task: t.clone() }),
            (State::Idle, Request::Cancel) => Ok(State::Idle),
            (State::Running { task: _ }, Request::Cancel) => Ok(State::Idle),
            (_, Request::Shutdown) => Ok(State::Stop),
            _ => Err(format!(
                "invalid transition, event: {event:?}, state: {self:?}"
            )),
        }
    }
}

pub mod error {
    //! Worker related errors

    use std::{
        error::Error,
        fmt,
    };

    /// Error produced by the `Worker`
    #[derive(PartialEq, Eq, Clone, Copy)]
    pub struct WorkerError<T>(pub T);

    impl<T> fmt::Debug for WorkerError<T> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("WorkerError").finish_non_exhaustive()
        }
    }

    impl<T: fmt::Display> fmt::Display for WorkerError<T> {
        fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(fmt, "worker error {}", self.0)
        }
    }

    impl<T: fmt::Display> Error for WorkerError<T> {}
}
