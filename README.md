# Rust Worker Pool Library

A Rust library for managing a pool of workers that can execute tasks concurrently. This library aims to provide a robust, scalable, and maintainable solution for managing worker lifecycles, task assignments, and error handling in a concurrent environment.

## Overview

Each worker operates as a state machine with defined states and transitions, ensuring predictable behavior and easy recovery from errors. It is inspired by Erlang's [poolboy](https://github.com/devinus/poolboy) library, bringing similar functionality to the Rust ecosystem. Workers are checked in/out by the supervisors depending on the task queue and their state.

## Goals 

- **Worker Pool Management**: Dynamically manage a pool of workers.
- **Task Queue**: Handle task assignments and maintain a queue for pending tasks.
- **State Machine for Workers**: Define clear states and transitions for workers.
- **Error Handling**: Robust error handling and recovery mechanisms.
- **Graceful Shutdown**: Ensure workers can shut down gracefully, completing or canceling tasks as needed.
- **Bi-directional Communication**: Facilitate communication between supervisor and workers.
- **Scalable** (ambitious): Abstract communication to support distributed workers

## Checklist of Features/Functionality

Anything that is marked as "complete" is mostly just mvp/proof-of-concept.

- [x] Basic Worker Pool Structure
- [x] Task Queue Implementation
- [x] Worker State Machine Design
- [x] Bi-directional Communication between Supervisor and Workers
- [x] Worker Lifecycle Management 
- [x] Error Handling and Recovery
- [x] Graceful Shutdown Process
- [ ] Dynamic Scaling of Worker Pool
- [ ] Configuration Options for Pool Size and Channel Sizes
- [x] Documentation and Examples

#### Future goals

There are a couple of major changes I'd like to make if things do progress.

* Refactor communication to purely rpc. This will enable remote-workers, more
  inline with erlang's distributed, fault-tolerant, scalable systems. It will
  also do away with existing hacks to get results propagated back to clients.

* Supervisor trees. Right now we are assuming a single supervisor, with a 
  flat hierarachy of workers. This should change to effectively support a 
  supervision tree.

Contributions are welcome!

## Usage

### Example

```rust
#[derive(Debug)]
struct TestWorker;

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

#[tokio::main]
async fn main() {
    let mut pool: Supervisor<TestWorker> = Supervisor::new(5);
    let task = TestWorkerTask {
        ctx: "test-worker".to_string(),
    };

    pool.enqueue(task).await;
    pool.run().await;
}
```
