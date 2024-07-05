mod bus;

use bus::{Bus, BusChannel};
use std::{collections::VecDeque, thread};

pub trait Worker: Send {
    type Task;
    type Output;
    type Error;

    fn spawn(id: usize, channel: BusChannel<Message>);
    fn run(&self) -> Result<Self::Output, Self::Error>;
    fn cancel(&self);
}

#[allow(dead_code)]
#[derive(Clone)]
struct TestWorker {
    id: usize,
    channel: BusChannel<Message>,
}

impl Worker for TestWorker {
    type Task = TestWorkerTask;
    type Output = ();
    type Error = &'static str;

    fn spawn(id: usize, channel: BusChannel<Message>) {
        thread::spawn(move || {
            let worker = Self { id, channel };
            let _ = worker.run();
        });
    }

    fn run(&self) -> Result<Self::Output, Self::Error> {
        loop {
            match self.channel.1.recv() {
                Ok(msg) => {
                    println!("got msg: {msg:?}");
                }
                Err(e) => {
                    eprintln!("{e}");
                }
            }
        }
    }

    fn cancel(&self) {}
}

#[allow(dead_code)]
struct TestWorkerTask {
    ctx: String,
}

#[allow(dead_code)]
pub struct Supervisor<W: Worker> {
    pub workers: Vec<usize>,
    pub queue: VecDeque<W::Task>,
    pub size: usize,
    bus: Bus<Message>,
}

#[derive(Debug)]
pub enum Message {
    Request(Request),
    Response(Response),
}

#[derive(Debug)]
pub enum Request {
    State,
}

#[derive(Debug)]
pub enum Response {
    State(bool),
}

#[allow(dead_code)]
impl<W: Worker> Supervisor<W> {
    fn new(size: usize) -> Self {
        let bus = Bus::new(size);
        // NOTE(jdb): For the time being we fill up the pool.
        let workers = (0..size)
            .map(|id| {
                W::spawn(id, bus.clone());
                id
            })
            .collect();

        Self {
            workers,
            queue: VecDeque::new(),
            size,
            bus,
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
            if let Some(_task) = self.queue.pop_back() {
                // Let's try to find a worker
                let msg = Message::Request(Request::State);
                println!("supervisor sent msg: {msg:?}");

                let send = self.bus.tx.send(msg);
                println!("supervisor send outcome {send:?}");
            }
        }
    }

    fn enqueue(&mut self, task: W::Task) {
        self.queue.push_back(task);
    }

    fn shutdown(&self) {
        // Emit a cancellation message and wait for all the
        // threads to ack and return
        unimplemented!()
    }
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
