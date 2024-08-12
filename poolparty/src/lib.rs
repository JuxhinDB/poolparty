pub mod message;
pub mod supervisor;
pub mod worker;

pub use crate::{
    message::Response,
    supervisor::Supervisor,
    worker::{Task, Workable},
};

// NOTE(jdb): This is just a temporary alias to signal that we want messages
// to always contain their Pid -- which ideally can be either a usize (local),
// or an ip (network) worker. This just aims to follow Erlang's message format.
pub type Pid = usize;
