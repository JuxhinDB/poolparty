use crate::worker::Workable;

// FIXME(jdb): Should really revise the naming. This feels
// half-arsed.
#[derive(Debug, Clone)]
pub enum SupervisorMessage<W: Workable> {
    State,
    Task(W::Task),
    Cancel,

    /// Acknowledge
    SubscribeAck,
    Shutdown,
}

#[derive(Debug)]
pub enum WorkerMessage<W: Workable> {
    Complete(Result<W::Output, (W::Error, W::Task)>),
    /// Worker subscribing to a running Supervisor.
    Subscribe,
    ShutdownAck, // Acknowledge shutdown
}
