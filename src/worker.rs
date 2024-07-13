use std::{fmt::Debug, future::Future, marker::PhantomData};

use crate::Pid;
use tokio::sync::mpsc::{Receiver, Sender};

pub trait Task: Send + Debug {}

/// A long-lived Worker/Actor that is sent tasks to execute and emit back events
/// to be handled by the supervisor.
pub trait Workable: Send + Sized {
    //  In the future, we may consider a way to reuse the existing worker
    //  by restarting it with the same task with a clean context.
    type Task: Task + Debug;
    type Output;
    type Error;

    fn process(task: Self::Task) -> impl Future<Output = Response<Self>>;
}

pub struct Worker<'a, W: Workable> {
    id: usize,
    rx: Receiver<Request<W>>,
    state: State<W>,
    worker: PhantomData<&'a W>,
}

impl<'a, W: Workable> Worker<'a, W> {
    fn new(id: usize, tx: Sender<(Pid, Response<W>)>, rx: Receiver<Request<W>>) -> Self {
        Self {
            id,
            rx,
            state: State::Idle,
            worker: PhantomData,
        }
    }

    async fn run(mut self) {
        loop {
            if let Some(_msg) = self.rx.recv().await {
                todo!()
            }
        }
    }
}

enum State<W: Workable> {
    Idle,
    Running { task: W::Task },
    Error(W::Error),
    Stop,
}

#[derive(Debug)]
pub enum Request<W: Workable> {
    State,
    Task(W::Task),
    Complete(Result<W::Output, W::Error>),
    Cancel,
    Shutdown,
}

#[derive(Debug)]
pub enum Response<W: Workable> {
    Complete(Result<W::Output, W::Error>),
}

impl<W: Workable> State<W> {
    // NOTE(jdb): The following is the initial state machine for workers.
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
    fn next(self, event: Request<W>) -> Result<State<W>, String> {
        match (self, event) {
            (State::Idle, Request::Task(t)) => Ok(State::Running { task: t }),
            (State::Idle, Request::Cancel) => Ok(State::Idle),
            (State::Running { task: _ }, Request::Complete(result)) => match result {
                Ok(_output) => Ok(State::Idle),
                Err(error) => Ok(State::Error(error)),
            },
            (State::Running { task: _ }, Request::Cancel) => Ok(State::Idle),
            (_, Request::Shutdown) => Ok(State::Stop),
            _ => Err("invalid transition".to_string()),
        }
    }

    fn state<'a>(&'a self) -> &'a State<W> {
        self
    }
}

pub mod error {
    //! Worker related errors

    use std::error::Error;
    use std::fmt;

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
