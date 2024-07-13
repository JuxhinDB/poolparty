use crate::worker::Workable;

#[derive(Debug)]
pub enum Message<W: Workable> {
    Request(Request<W>),
    Response(Response<W>),
}

#[derive(Debug)]
pub enum Request<W: Workable> {
    State,
    Task(W::Task),
    Complete(Result<W::Output, W::Error>),
    Cancel,
    Shutdown,
}

#[derive(Debug)]
pub enum Response<W: Workable> {
    Complete(Result<W::Output, W::Error>),
}
