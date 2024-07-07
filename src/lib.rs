use std::{
    collections::VecDeque,
    fmt::Debug,
    sync::mpsc::{self, Receiver, Sender},
    thread,
};

// NOTE(jdb): This is just a temporary alias to signal that we want messages
// to always contain their Pid -- which ideally can be either a usize (local),
// or an ip (network) worker. This just aims to follow Erlang's message format.
pub type Pid = usize;

#[allow(dead_code)]
pub struct Supervisor<W: Worker> {
    // Internal worker pool, containing the queue of workers that are ready
    // to receive a task (i.e., checkout).
    pool: VecDeque<(Pid, Sender<Message<W::Task>>)>,

    // An internal pool containing the list of checked out workers. We need
    // to do this in order to keep channels alive and keep communication with
    // workers even as they are running.
    checked: VecDeque<(Pid, Sender<Message<W::Task>>)>,

    queue: VecDeque<W::Task>,
    size: usize,
}

/// A long-lived Worker/Actor that is sent tasks to execute and emit back events
/// to be handled by the supervisor.
pub trait Worker: Send {
    type Task: Task + Debug;
    type Output;
    type Error;

    // The worker is spawned and in an `idle` state. At this stage it is simply
    // waiting on the supervisor to check it out of the pool with a task.
    //
    // Currently this is all the worker needs. It should listen to messages on
    // the channel to determine the next course of action.
    //
    // FIXME(jdb): This should be fallible
    fn spawn(id: usize, rx: Receiver<Message<Self::Task>>);
}

#[allow(dead_code)]
impl<W: Worker> Supervisor<W> {
    fn new(size: usize) -> Self {
        let mut pool = VecDeque::with_capacity(size);

        for id in 0..size {
            let (tx, rx) = mpsc::channel();
            W::spawn(id, rx);
            pool.push_front((id, tx));
        }

        Self {
            pool,
            checked: VecDeque::with_capacity(size),
            queue: VecDeque::new(),
            size,
        }
    }

    fn run(&mut self) {
        // Start running the supervisor manage worker lifecycle
        //
        // This method will continuously check the task queue and assign tasks
        // to available workers until all tasks are processed or a shutdown
        // signal is received.
        //
        // Dynamically spawn workers if the pool is not at full capacity when
        // tasks are enqueued.
        loop {
            if let Some(task) = self.queue.pop_back() {
                // We want to check if there is a worker available in the pool,
                // if not we have two options:
                //
                // 1. Spawn a new worker if we are within capacity limits;
                // 2. Wait until the next worker is available.
                if let Some(worker) = self.pool.pop_front() {
                    // Let's try to find a worker
                    let msg = Message::Request(Request::Task(task));
                    println!("supervisor sent msg: {msg:?}");

                    // We should only work with Workers that are available. If the receiver
                    // has dropped, then we should drop this worker entirely from the pool.
                    //
                    // This _couold_ lead to
                    if worker.1.send(msg).is_ok() {
                        // Move the worker to the checked out pool so that the channel
                        // remains open.
                        self.checked.push_front(worker);
                    }
                }
            }
        }
    }

    fn enqueue(&mut self, task: W::Task) {
        self.queue.push_back(task);
    }

    fn shutdown(&self) {
        // Emit a cancellation message and wait for all the
        // workers to ack or timeout.
        unimplemented!()
    }
}

// NOTE(jdb): How should we go about embedding a Ctx in here that is flexible
// for the implementer?
pub trait Task: Send + Debug {}

#[allow(dead_code)]
#[derive(Debug)]
struct TestWorker {
    id: usize,
    rx: Receiver<Message<TestWorkerTask>>,
}

#[allow(dead_code)]
#[derive(Debug)]
struct TestWorkerTask {
    ctx: String,
}

impl Task for TestWorkerTask {}

impl Worker for TestWorker {
    type Task = TestWorkerTask;
    type Output = ();
    type Error = &'static str;

    fn spawn(id: usize, rx: Receiver<Message<Self::Task>>) {
        thread::spawn(move || {
            let worker = Self { id, rx };

            loop {
                match worker.rx.recv() {
                    Ok(msg) => {
                        println!("got msg: {msg:?}");
                    }
                    Err(e) => {
                        eprintln!("{e}");
                    }
                }
            }
        });
    }
}

#[derive(Debug)]
pub enum Message<T: Task> {
    Request(Request<T>),
    Response(Response),
}

#[derive(Debug)]
pub enum Request<T: Task> {
    State,
    Task(T),
}

#[derive(Debug)]
pub enum Response {
    State(bool),
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn usage_test() {
        let mut pool: Supervisor<TestWorker> = Supervisor::new(5);
        let task = TestWorkerTask {
            ctx: "test-worker".to_string(),
        };

        pool.enqueue(task);
        pool.run();
    }
}
