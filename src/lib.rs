pub mod message;
pub mod supervisor;
pub mod worker;

use std::fmt::Debug;

use tokio::sync::mpsc::{Receiver, Sender};

use crate::{
    message::{Message, Response},
    worker::{Task, Workable},
};

// NOTE(jdb): This is just a temporary alias to signal that we want messages
// to always contain their Pid -- which ideally can be either a usize (local),
// or an ip (network) worker. This just aims to follow Erlang's message format.
pub type Pid = usize;

#[cfg(test)]
mod test {
    use super::*;
    use supervisor::Supervisor;

    #[tokio::test]
    async fn usage_test() {
        let mut pool: Supervisor<TestWorker> = Supervisor::new(5);
        let task = TestWorkerTask {
            ctx: "test-worker".to_string(),
        };

        pool.enqueue(task).await;
        pool.run().await;
    }
}
