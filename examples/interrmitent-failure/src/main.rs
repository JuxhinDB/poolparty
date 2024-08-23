use std::time::Duration;

use poolparty::{buffer::RingBuffer, Supervisor, Task, Workable};

use tokio::sync::mpsc::Sender;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[derive(Debug, Default, Clone)]
struct InterrmitentFailureTask {
    counter: usize,
}

impl Task for InterrmitentFailureTask {}

#[derive(Debug)]
struct InterrmitentFailureWorker;

impl Workable for InterrmitentFailureWorker {
    type Task = InterrmitentFailureTask;

    type Output = ();

    type Error = ();

    async fn process(_task: Self::Task) -> Result<Self::Output, Self::Error> {
        tokio::time::sleep(Duration::from_millis(250)).await;

        if coinflip::flip() {
            Ok(())
        } else {
            Err(())
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    tracing::info!("starting intermittent-failure example...");

    let buffer = RingBuffer::new();
    let mut supervisor: Supervisor<InterrmitentFailureWorker> = Supervisor::new(5, &buffer);

    let queue_tx = supervisor.queue.0.clone();

    tokio::select! {
        _ = supervisor.run() => {},
        _ = do_work(queue_tx) => {},
        _ = tokio::signal::ctrl_c() => {
            supervisor.shutdown().await;
            return Ok(());
        },
    }

    Ok(())
}

async fn do_work(tx: Sender<InterrmitentFailureTask>) {
    let mut task = InterrmitentFailureTask::default();
    for _ in 0..=100 {
        task.counter += 1;
        let _ = tx.send(task.clone()).await;
    }

    // Keep the task running to avoid breaking the select! before
    // any of the work is completed.
    tokio::time::sleep(Duration::from_secs(10)).await;
}
