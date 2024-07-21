use std::{fmt::Debug, future::Future, marker::PhantomData};

use crate::{
    message::{Request, Response},
    Pid,
};
use tokio::sync::mpsc::{Receiver, Sender};

pub trait Task: Send + Debug {}

/// A long-lived Worker/Actor that is sent tasks to execute and emit back events
/// to be handled by the supervisor.
pub trait Workable: Debug + Send + Sync + Sized {
    //  In the future, we may consider a way to reuse the existing worker
    //  by restarting it with the same task with a clean context.
    type Task: Task + Debug;
    type Output: Debug + Send;
    type Error: Debug + Send;

    fn process(task: Self::Task) -> impl Future<Output = Response<Self>>;
}

pub struct Worker<'a, W: Workable> {
    id: usize,
    rx: Receiver<Request<W>>,
    state: State<W>,
    worker: PhantomData<&'a W>,
}

impl<'a, W: Workable> Worker<'a, W> {
    pub fn new(id: usize, tx: Sender<(Pid, Response<W>)>, rx: Receiver<Request<W>>) -> Self {
        Self {
            id,
            rx,
            state: State::Idle,
            worker: PhantomData,
        }
    }

    fn handle_event(&mut self, event: Request<W>) -> Result<(), String> {
        match self.state.next(event) {
            Ok(state) => {
                // We've successfully transitioned our state and
                // should handle the transition accordingly. I don't
                // quite like how the state handling is split up,
                // it's really ugly. But we make it work first, then
                // we make it fast/pretty.
                match &state {
                    State::Running { task } => {
                        // TODO(jdb): Run the task
                        println!("running task {task:?}");
                    }
                    State::Idle => {
                        // Do nothing, wait for a new task
                    }
                    State::Error(err) => {
                        // Something bad happened
                        eprintln!("error during execution: {err:?}");
                    }
                    State::Stop => {
                        // NOTE(jdb): need some `tx` back to supervisor
                        println!("worker shutting down...");
                        return Ok(());
                    }
                }

                // NOTE(jdb): This is code smell, as we should be
                // transitioning the state before anything else.
                //
                // I need to figure out how to enable the mutation
                // of this field without moving. Projection might
                // not work here due to unsized enum variants
                self.state = state;

                Ok(())
            }
            Err(invalid_transition) => {
                // FIXME(jdb): Propagate error
                eprintln!("err: {invalid_transition:?}, skipping message");

                Err(invalid_transition)
            }
        }
    }

    pub async fn run(mut self) {
        loop {
            tokio::select! {
                Some(event) = self.rx.recv() => {
                    println!("received event {event:?}");

                    if self.handle_event(event).is_ok() {
                        println!("transitioned to state {state:?}", state = self.state);
                    }
                }
                else => {
                    eprintln!("channel closed unexpectedly");
                }
            }
        }
    }
}

#[derive(Debug)]
enum State<W: Workable> {
    Idle,
    Running { task: W::Task },
    Error(W::Error),
    Stop,
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
    fn next(&self, event: Request<W>) -> Result<State<W>, String> {
        match (self, event) {
            (State::Idle, Request::Task(t)) => Ok(State::Running { task: t }),
            (State::Idle, Request::Cancel) => Ok(State::Idle),
            (State::Running { task: _ }, Request::Cancel) => Ok(State::Idle),
            (_, Request::Shutdown) => Ok(State::Stop),
            _ => Err("invalid transition".to_string()),
        }
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
