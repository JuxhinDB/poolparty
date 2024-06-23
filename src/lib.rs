mod bus;

use std::collections::VecDeque;

use bus::Bus;

pub trait Worker: Send {
    type Task;
    type Output;
    type Error;

    fn spawn(id: usize) -> Self;
    fn run(&self, task: Self::Task) -> Result<Self::Output, Self::Error>;
    fn cancel(&self);
}

#[allow(dead_code)]
#[derive(Clone, Copy)]
struct TestWorker {
    id: usize,
}

impl Worker for TestWorker {
    type Task = TestWorkerTask;
    type Output = ();
    type Error = &'static str;

    fn spawn(id: usize) -> Self {
        Self { id }
    }

    fn run(&self, _task: Self::Task) -> Result<Self::Output, Self::Error> {
        Ok(())
    }

    fn cancel(&self) {}
}

#[allow(dead_code)]
struct TestWorkerTask {
    ctx: String,
}

#[allow(dead_code)]
pub struct Supervisor<W: Worker> {
    pub workers: Vec<Box<W>>,
    pub queue: VecDeque<W::Task>,
    pub size: usize,
    bus: Bus,
}

#[allow(dead_code)]
impl<W: Worker> Supervisor<W> {
    fn new(size: usize) -> Self {
        Self {
            workers: Vec::with_capacity(size),
            queue: VecDeque::new(),
            size,
            bus: Bus::new(size),
        }
    }

    fn run(&mut self) {
        // Start running the supervisor manage worker lifecycle
        todo!()
    }

    fn enqueue(&mut self, task: W::Task) {
        self.queue.push_back(task);
    }

    fn shutdown(&self) {
        for worker in &self.workers {
            worker.cancel();
        }
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
    }
}
