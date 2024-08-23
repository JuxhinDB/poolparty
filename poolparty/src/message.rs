use crate::worker::Workable;

#[derive(Debug, Clone)]
pub enum Request<W: Workable> {
    State,
    Task(W::Task),
    Cancel,
    Shutdown,
}

#[derive(Debug)]
pub enum Response<W: Workable> {
    ShutdownAck, // Acknowledge shutdown
    Complete(Result<W::Output, (W::Error, W::Task)>),
}
