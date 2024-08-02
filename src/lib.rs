pub mod message;
pub mod supervisor;
pub mod worker;

use std::fmt::Debug;

use crate::{
    message::Response,
    worker::{Task, Workable},
};

// NOTE(jdb): This is just a temporary alias to signal that we want messages
// to always contain their Pid -- which ideally can be either a usize (local),
// or an ip (network) worker. This just aims to follow Erlang's message format.
pub type Pid = usize;

#[derive(Debug)]
struct TestWorker;

#[allow(dead_code)]
#[derive(Debug, Clone)]
struct TestTask {
    msg: String,
}

impl Task for TestTask {}

impl Workable for TestWorker {
    type Task = TestTask;
    type Output = String;
    type Error = String;

    async fn process(task: Self::Task) -> Response<Self> {
        Response::Complete(Ok(format!("got task {task:?}")))
    }
}

#[derive(Debug)]
struct ErrorTestWorker;

impl Workable for ErrorTestWorker {
    type Task = TestTask;
    type Output = String;
    type Error = String;

    async fn process(_: Self::Task) -> Response<Self> {
        Response::Complete(Err("simulated error".to_string()))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use supervisor::Supervisor;

    #[tokio::test]
    async fn usage_test() {
        // impl Workable for TestWorker {}
        //
        // let worker: Worker<TestWorker> = Worker::new(1, tx, rx);
        let mut pool: Supervisor<TestWorker> = Supervisor::new(1);

        let task = TestTask {
            msg: "hello-world".to_string(),
        };

        pool.enqueue(task.clone()).await.unwrap();

        // This task does not get processed yet as the pool size is 1
        pool.enqueue(task).await.unwrap();

        run(pool).await;
    }

    #[tokio::test]
    async fn error_test() {
        let mut pool: Supervisor<ErrorTestWorker> = Supervisor::new(1);

        let task = TestTask {
            msg: "hello-world".to_string(),
        };

        pool.enqueue(task.clone()).await.unwrap();

        // This task does not get processed yet as the pool size is 1
        pool.enqueue(task).await.unwrap();
        run(pool).await;
    }

    #[tokio::test]
    async fn multiple_worker_test() {
        // impl Workable for TestWorker {}
        //
        // let worker: Worker<TestWorker> = Worker::new(1, tx, rx);
        let mut pool: Supervisor<TestWorker> = Supervisor::new(4);

        let task = TestTask {
            msg: "hello-world".to_string(),
        };

        pool.enqueue(task.clone()).await.unwrap();
        pool.enqueue(task.clone()).await.unwrap();
        pool.enqueue(task.clone()).await.unwrap();

        // This task does not get processed yet as the pool size is 1
        pool.enqueue(task).await.unwrap();

        run(pool).await;
    }

    async fn run<W: Workable + 'static>(mut pool: Supervisor<W>) {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!("received shutdown signal");
                pool.shutdown().await;
            },
            _ = pool.run() => {

            }
        }
    }
}
