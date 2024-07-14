use crate::worker::Workable;

#[derive(Debug)]
pub enum Request<W: Workable> {
    State,
    Task(W::Task),
    Cancel,
    Shutdown,

    // NOTE(jdb): This should only be in the `Response`, there's no
    // reason for this to be kept in `Request`.
    Complete(Result<W::Output, W::Error>),
}

#[derive(Debug)]
pub enum Response<W: Workable> {
    Complete(Result<W::Output, W::Error>),
}
