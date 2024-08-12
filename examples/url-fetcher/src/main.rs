use poolparty::{Supervisor, Task, Workable};
use reqwest::{StatusCode, Url};

use anyhow::Context;

use std::io;

#[derive(Debug, Clone)]
struct UrlFetchTask {
    url: Url,
}

impl Task for UrlFetchTask {}

#[derive(Debug)]
struct UrlFetchWorker;

impl Workable for UrlFetchWorker {
    type Task = UrlFetchTask;
    type Output = String;
    type Error = anyhow::Error;

    async fn process(task: Self::Task) -> Result<Self::Output, Self::Error> {
        let response = reqwest::get(task.url.clone())
            .await
            .context("error fetching url")?;

        let prefix = if response.status() == StatusCode::OK {
            "ヽ༼ ಠ_ಠ༽ﾉ"
        } else {
            "(╥﹏╥)"
        };

        Ok(format!(
            "{prefix} fetching {url} returned {status}",
            url = task.url,
            status = response.status()
        ))
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut supervisor: Supervisor<UrlFetchWorker> = Supervisor::new(5);
    let queue_tx = supervisor.queue.0.clone();

    tokio::spawn(async move {
        supervisor.run().await;
    });

    loop {
        if let Some(input) = get_input() {
            let task = UrlFetchTask {
                url: input.parse().expect("invalid url"),
            };

            for _ in 0..=100 {
                let _ = queue_tx.send(task.clone()).await;
            }
        }
    }
}

fn get_input() -> Option<String> {
    println!("Enter a url you'd like to enqueue a url fetch for: ");
    let mut input = String::new();

    if io::stdin().read_line(&mut input).is_ok() {
        Some(input.trim().to_string())
    } else {
        None
    }
}
