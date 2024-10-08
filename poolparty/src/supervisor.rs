use std::{collections::VecDeque, num::NonZeroUsize, time::Duration};

use error::SupervisorError;
use tokio::{
    sync::mpsc::{self, Receiver, Sender},
    task::JoinHandle,
};

use crate::{
    bounded_map::BoundedBTreeMap,
    buffer::RingBuffer,
    message::{Request, Response},
    worker::{Workable, Worker},
    Pid,
};

// Convenient type-alias to avoid overly complex types
#[allow(type_alias_bounds)]
type ResultRingBuffer<W: Workable> = RingBuffer<Result<W::Output, (W::Error, W::Task)>>;

// Convenient type-alias storing the worker ctx for the supervisor containing
// the sender and task handle.
#[allow(type_alias_bounds)]
type WorkerCtx<W: Workable> = (Sender<Request<W>>, JoinHandle<()>);

#[derive(Debug)]
pub struct Supervisor<'buf, W: Workable> {
    /// Internal worker pool, containing the queue of workers that are ready
    /// to receive a task (i.e., checkout).
    pool: BoundedBTreeMap<Pid, WorkerCtx<W>>,

    /// An internal pool containing the list of checked out workers. We need
    /// to do this in order to keep channels alive and keep communication with
    /// workers even as they are running.
    checked: BoundedBTreeMap<Pid, WorkerCtx<W>>,

    /// Pending queue of tasks to be executed
    tasks: VecDeque<W::Task>,

    /// Queue of Tasks to be sent out.
    pub queue: (Sender<W::Task>, Receiver<W::Task>),

    /// Buffer of worker results
    pub results: &'buf ResultRingBuffer<W>,

    /// Sender end, mostly kept here for the time being to simplify spawning.
    sender: Sender<(Pid, Response<W>)>,

    /// Receiver end of the channel between all workers and the supervisor. This
    /// allows workers to emit messages back to the supervisor efficiently.
    receiver: Receiver<(Pid, Response<W>)>,
}

impl<'buf, W: Workable + 'static> Supervisor<'buf, W> {
    pub fn new(size: NonZeroUsize, buffer: &'buf ResultRingBuffer<W>) -> Self {
        let pool = BoundedBTreeMap::new(size);
        let (supervisor_tx, supervisor_rx) = mpsc::channel(1024);

        let mut supervisor = Self {
            pool,
            checked: BoundedBTreeMap::new(size),
            tasks: VecDeque::new(),
            queue: mpsc::channel(1024),
            results: buffer,
            sender: supervisor_tx.clone(),
            receiver: supervisor_rx,
        };

        // FIXME(jdb): Remove this, workers should not be spawned on
        // initialisation of the supervisor
        supervisor.spawn(size.into()).unwrap();

        supervisor
    }

    #[tracing::instrument(skip(self),
        fields(
            pending_tasks = self.tasks.len(),
            workers_in_pool = self.pool.len(),
            checked_in_workers = self.checked.len(),
            new_workers = n,
        )
    )]
    fn spawn(&mut self, n: usize) -> Result<(), SupervisorError> {
        for _ in 0..n {
            let id = uuid::Uuid::new_v4();
            let supervisor_tx = self.sender.clone();
            let (worker_tx, worker_rx) = mpsc::channel(1024);

            let handle = tokio::spawn(async move {
                Worker::new(id, supervisor_tx.clone(), worker_rx)
                    .run()
                    .await;
            });

            tracing::info!("spawning worker with id {id}");
            self.pool.insert(id, (worker_tx, handle))?;
        }

        Ok(())
    }

    #[tracing::instrument(skip_all)]
    pub async fn run(&mut self) {
        // Start running the supervisor manage worker lifecycle
        //
        // This method will continuously check the task queue and assign tasks
        // to available workers until all tasks are processed or a shutdown
        // signal is received.
        //
        // Dynamically spawn workers if the pool is not at full capacity when
        // tasks are enqueued.
        tracing::info!("starting supervisor...");

        // In the event that the queue has one or more tasks pending (i.e., due
        // to not having any available workers), with no new tasks coming in,
        // we want to ensure that we still periodically check the task queue
        // given that the pool size of >=0.
        //
        // NOTE(jdb): I'd prefer a better alternative to polling the queue
        // every tick. Some form of notify mechanism may be more suitable.
        let mut task_queue_interval = tokio::time::interval(Duration::from_millis(250));
        task_queue_interval.tick().await;

        loop {
            tokio::select! {
                // New task has been enqueued
                task = self.queue.1.recv() => {
                    if let Some(task) = task {
                        tracing::trace!("enqueuing task {task:?}");
                        self.tasks.push_back(task);
                    } else {
                        tracing::error!("internal task queue closed unexpectedly");
                    }
                },
                // FIXME(jdb): Curently this assumes that the supervisor pool
                // is pre-allocated with the maximum number of workers. This
                // shouldn't be the case, as we may need to spawn one or more
                // workers.
                _ = task_queue_interval.tick(), if !self.tasks.is_empty() => {
                    // FIXME(jdb): We should aim to allocate as many tasks as
                    // possible from the task queue to our worker pool. Currently
                    // we are only allocating one task at a time.
                    if let (Some(worker), Some(task)) = (self.pool.pop_first(), self.tasks.pop_front()) {
                        let msg = Request::Task(task);

                        if worker.1.0.send(msg).await.is_ok() {
                            // Move the worker to the checked out pool so that
                            // the channel remains open.
                            if let Err(e) = self.checked.insert(worker.0, worker.1) {
                                tracing::error!("unable to move worker to checked in pool as it has reached capacity: {e}");
                            }
                        }
                    }
                },
                // Received a message from one of our workers
                msg = self.receiver.recv() => {
                    match msg {
                        Some((pid, Response::Complete(result))) => {
                            tracing::debug!("received msg from worker {pid}: {result:?}");

                            match &result {
                                Ok(_) => {
                                    // Place the worker back in the pool
                                    if let Some(worker) = self.checked.remove_entry(&pid) {
                                        if let Err(e) = self.pool.insert(worker.0, worker.1) {
                                            tracing::error!("unable to place worker back into ready pool as it has reached capacity: {e}");
                                        }
                                    }
                                },
                                Err((err, task)) => {
                                    tracing::info!("received error from worker {pid}, err: {err:?}, retrying task: {task:?}");

                                    // Remove the worker from the checked pool and kill/drop it.
                                    //
                                    // We should then spawn a new worker in the pool to be able to
                                    // take it's place.
                                    if let Some(worker) = self.checked.remove_entry(&pid) {
                                        let spawn = self.spawn(1);
                                        if let Err(SupervisorError::CapacityLimit(_)) = spawn {
                                            tracing::warn!("unable to spawn worker due to reaching pool limit {}", self.pool.len());
                                        } else if let Err(e) = spawn {
                                            tracing::error!("error while spawning worker {e:?}");
                                        }


                                        drop(worker);
                                    }

                                    // Try to place the task back to the front of the queue.
                                    //
                                    // NOTE(jdb): It's unclear if pushing to the front of the task
                                    // queue is the smart strategy here. There is a chance that a
                                    // broken task definition/state may cause repeated failures.
                                    //
                                    // In the future we should keep a retry counter to avoid this.
                                    self.tasks.push_front(task.to_owned());
                                }
                            }

                            if let Err(e) = self.results.push(result) {
                                tracing::error!("fatal error pushing worker result to ring buffer: {e:?}");
                            }

                        },
                        Some((pid, res)) => {
                            tracing::debug!("received res from worker {pid}: {res:?}");
                        }
                        None => {
                            panic!("no workers running");
                        }
                    }
                }
            };
        }
    }

    #[tracing::instrument(skip(self),
        fields(
            pending_tasks = self.tasks.len(),
            workers_in_pool = self.pool.len(),
            checked_in_workers = self.checked.len(),
        )
    )]
    pub async fn shutdown(mut self) {
        // Emit a cancellation message and wait for all the
        // workers to ack or timeout.
        tracing::info!("shutting down supervisor");

        // FIXME(jdb): Clean up this mess
        let mut workers: BoundedBTreeMap<Pid, WorkerCtx<W>> =
            BoundedBTreeMap::new(NonZeroUsize::new(self.pool.len() * 2).unwrap());
        workers.append(&mut self.pool);
        workers.append(&mut self.checked);

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
                tracing::debug!("all workers have been shut down");
                break;
            }

            tokio::select! {
                msg = self.receiver.recv() => {
                    if let Some((worker_id, Response::ShutdownAck)) = msg {
                        tracing::debug!("got shutdown ack from {worker_id}");
                        workers.remove(&worker_id);
                    }
                },
                _ = timeout.tick() => {
                    tracing::error!("shutdown timeout elapsed, one or more workers may remain in an inconsistent state");
                    break;
                }
            }
        }
    }
}

pub mod error {
    //! Supervisor related errors
    use std::fmt;

    use thiserror::Error;

    use crate::bounded_map;

    /// Error produced by the `Supervisor`
    #[derive(Error, PartialEq, Eq, Clone, Copy)]
    pub enum SupervisorError {
        CapacityLimit(#[from] bounded_map::error::BoundedBTreeMapError),
    }

    impl fmt::Debug for SupervisorError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("SupervisorError").finish_non_exhaustive()
        }
    }

    impl fmt::Display for SupervisorError {
        fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(fmt, "SupervisorError")
        }
    }
}
