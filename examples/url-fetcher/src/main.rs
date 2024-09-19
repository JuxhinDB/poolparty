use std::num::NonZeroUsize;

use anyhow::Context;
use poolparty::{
    buffer::RingBuffer,
    Supervisor,
    Task,
    Workable,
};
use reqwest::{
    StatusCode,
    Url,
};
use tokio::io::{
    AsyncBufReadExt,
    BufReader,
};
use tracing_subscriber::{
    fmt,
    prelude::*,
    EnvFilter,
};

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
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    tracing::info!("starting url-fetch example...");

    let buffer = RingBuffer::new();
    let mut supervisor: Supervisor<UrlFetchWorker> =
        Supervisor::new(NonZeroUsize::new(5).unwrap(), &buffer);

    println!("Enter a url you'd like to enqueue a url fetch task for: ");
    let mut reader = BufReader::new(tokio::io::stdin());
    let mut input = String::new();

    if reader.read_line(&mut input).await.is_ok() {
        let task = UrlFetchTask {
            url: input.trim().to_string().parse().expect("invalid url"),
        };

        for _ in 0..=10 {
            let _ = supervisor.queue.0.send(task.clone()).await;
        }
    }

    tokio::select! {
        _ = supervisor.run() => {},
        _ = results(&buffer) => {}
        _ = tokio::signal::ctrl_c() => {
            supervisor.shutdown().await;
            return Ok(());
        },
    }

    Ok(())
}

async fn results(buffer: &RingBuffer<Result<String, (anyhow::Error, UrlFetchTask)>>) {
    loop {
        let msg = buffer.recv().await;
        println!("received a result {msg:?}");
    }
}
