use crate::{
    message::{Request, Response},
    worker::{Workable, Worker},
    Pid,
};
use std::{
    collections::{BTreeMap, VecDeque},
    time::Duration,
};

use tokio::{
    sync::mpsc::{self, Receiver, Sender},
    task::JoinHandle,
};

// Internal type alias holding the context needed to communicate
// as well as abort tasks.
//
// NOTE(jdb): It's unclear, design-wise, if we want to rely on the tokio
// `JoinHandle`. This will make running workers across network much more
// difficult and goes against the design-idea of the library.
type WorkerHandle<W> = (Pid, Sender<Request<W>>, JoinHandle<()>);

#[allow(dead_code)]
pub struct Supervisor<W: Workable> {
    // Internal worker pool, containing the queue of workers that are ready
    // to receive a task (i.e., checkout).
    pool: VecDeque<WorkerHandle<W>>,

    // An internal pool containing the list of checked out workers. We need
    // to do this in order to keep channels alive and keep communication with
    // workers even as they are running.
    checked: VecDeque<WorkerHandle<W>>,

    // Queue of Tasks to be sent out.
    pub queue: (Sender<W::Task>, Receiver<W::Task>),

    // Receiver end of the channel between all workers and the supervisor. This
    // allows workers to emit messages back to the supervisor efficiently.
    receiver: Receiver<(Pid, Response<W>)>,

    size: usize,
}

#[allow(dead_code)]
impl<W: Workable + 'static> Supervisor<W> {
    pub fn new(size: usize) -> Self {
        let mut pool = VecDeque::with_capacity(size);
        let (supervisor_tx, supervisor_rx) = mpsc::channel(1024);

        for id in 0..=size {
            let supervisor_tx = supervisor_tx.clone();
            let (tx, rx) = mpsc::channel(1024);

            let handle = tokio::spawn(async move {
                Worker::new(id, supervisor_tx.clone(), rx).run().await;
            });

            pool.push_front((id, tx, handle));
        }

        Self {
            pool,
            checked: VecDeque::with_capacity(size),
            queue: mpsc::channel(1024),
            receiver: supervisor_rx,
            size,
        }
    }

    pub async fn run(&mut self) {
        // Start running the supervisor manage worker lifecycle
        //
        // This method will continuously check the task queue and assign tasks
        // to available workers until all tasks are processed or a shutdown
        // signal is received.
        //
        // Dynamically spawn workers if the pool is not at full capacity when
        // tasks are enqueued.
        loop {
            tokio::select! {
                // NOTE(jdb): Consider `biased;` polling to make sure that
                // noisy workers do not prevent tasks from being enqueued.

                // New task has been enqueued
                task = self.queue.1.recv() => {
                    match task {
                        Some(task) => {
                            // We want to check if there is a worker available in the pool,
                            // if not we have two options:
                            //
                            // 1. Spawn a new worker if we are within capacity limits;
                            // 2. Wait until the next worker is available.
                            if let Some(worker) = self.pool.pop_front() {
                                // Let's try to find a worker
                                let msg = Request::Task(task);

                                // We should only work with Workers that are available. If the receiver
                                // has dropped, then we should drop this worker entirely from the pool.
                                //
                                // This _couold_ lead to some odd situation where a worker is left
                                // into some intermittent state.
                                if worker.1.send(msg).await.is_ok() {
                                    // Move the worker to the checked out pool so that the channel
                                    // remains open.
                                    self.checked.push_front(worker);
                                }
                            }
                        },
                        None => {
                            eprintln!("internal task queue closed unexpectedly");
                        }
                    }
                },
                // Received a message from one of our workers
                msg = self.receiver.recv() => {
                    match msg {
                        Some(msg) => {
                            println!("received msg from worker: {msg:?}");
                        },
                        None => {
                            eprintln!("no workers running");
                        }
                    }
                }
            };
        }
    }

    pub async fn shutdown(mut self) {
        // Emit a cancellation message and wait for all the
        // workers to ack or timeout.
        println!("shutting down supervisor");

        let mut workers: BTreeMap<Pid, (Sender<Request<W>>, JoinHandle<()>)> = self
            .pool
            .into_iter()
            .chain(self.checked.into_iter())
            .map(|w| (w.0, (w.1, w.2)))
            .collect();

        for worker in workers.iter() {
            let sender = &worker.1 .0; // FIXME(jdb): terrible, but lazy
            sender
                .send(Request::Shutdown)
                .await
                .expect("unable to send shutdown to worker {worker}");
        }

        let mut timeout = tokio::time::interval(Duration::from_secs(10));
        timeout.tick().await;

        loop {
            if workers.is_empty() {
                println!("all workers have been shutdown");
                break;
            }

            tokio::select! {
                msg = self.receiver.recv() => {
                    if let Some((worker_id, Response::ShutdownAck)) = msg {
                        println!("got shutdown ack from {worker_id}");
                        workers.remove(&worker_id);
                    }
                },
                _ = timeout.tick() => {
                    eprintln!("shutdown timeout elapsed, one or more workers may remain in an inconsistent state");
                    break;
                }
            }
        }
    }
}

pub mod error {
    //! Supervisor related errors

    use std::error::Error;
    use std::fmt;

    use tokio::sync::mpsc::error::SendError;

    use crate::worker::Task;

    /// Error produced by the `Supervisor`
    #[derive(PartialEq, Eq, Clone, Copy)]
    pub struct SupervisorError<T>(pub T);

    impl<T> fmt::Debug for SupervisorError<T> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("SupervisorError").finish_non_exhaustive()
        }
    }

    impl<T: fmt::Display> fmt::Display for SupervisorError<T> {
        fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(fmt, "supervisor error {}", self.0)
        }
    }

    impl<T: fmt::Display> Error for SupervisorError<T> {}

    #[derive(Debug)]
    pub struct EnqueueError<T: Task>(pub SendError<T>);

    impl<T: Task> From<SendError<T>> for EnqueueError<T> {
        fn from(value: SendError<T>) -> Self {
            Self(value)
        }
    }

    impl<T: Task + fmt::Debug> fmt::Display for EnqueueError<T> {
        fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(fmt, "error enqueuing task {self:?}")
        }
    }
}
