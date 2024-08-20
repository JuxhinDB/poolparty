//! Rudimentary ring buffer that is primarily a `VecDequeue` with `Notify`
//! added on top of it.
//!
//! The goal is to make it more convenient for downstream clients to consume
//! from the buffer efficiently.

use error::RingBufferError;
use std::{collections::VecDeque, ops::DerefMut, sync::Mutex};
use tokio::sync::Notify;

#[derive(Debug)]
pub struct RingBuffer<T> {
    // FIXME(jdb): Make this inner buffer bounded
    inner: Mutex<VecDeque<T>>,
    notify: Notify,
}

impl<T> RingBuffer<T> {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(VecDeque::new()),
            notify: Notify::new(),
        }
    }

    pub fn push(&self, value: T) -> Result<(), RingBufferError<T>> {
        self.inner.lock()?.push_back(value);

        // Notify all the waiters that a new value is available
        self.notify.notify_waiters();

        Ok(())
    }

    fn try_recv(&self) -> Result<VecDeque<T>, RingBufferError<T>> {
        let mut locked_queue = self.inner.lock()?;

        // We replace the existing buffer with an empty buffer. We do
        // this to bulk remove all values from the buffer rather than
        // popping values one by one.
        let values = std::mem::take(locked_queue.deref_mut());

        Ok(values)
    }

    pub async fn recv(&self) -> Result<VecDeque<T>, RingBufferError<T>> {
        let future = self.notify.notified();
        tokio::pin!(future);

        loop {
            // Make sure that no wakeup is lost if we get
            // `None` from `try_recv`.
            future.as_mut().enable();

            let values = self.try_recv()?;

            if !values.is_empty() {
                return Ok(values);
            } else {
                tracing::warn!(
                    "calling of `RingBuffer::recv` resulted in an \
                    empty buffer. this should not happen as the task is \
                    notified only when one or more values are available."
                );
            }

            future.as_mut().await;

            future.set(self.notify.notified());
        }
    }
}

pub(crate) mod error {

    use std::{
        collections::VecDeque,
        error::Error,
        fmt,
        sync::{MutexGuard, PoisonError},
    };

    pub enum RingBufferError<'a, T> {
        PoisonedLock(PoisonError<MutexGuard<'a, VecDeque<T>>>),
    }

    impl<'a, T> fmt::Debug for RingBufferError<'a, T> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_struct("RingBufferError").finish_non_exhaustive()
        }
    }

    impl<'a, T: fmt::Display> fmt::Display for RingBufferError<'a, T> {
        fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(fmt, "ring buffer error {:?}", self)
        }
    }

    impl<'a, T: fmt::Display> Error for RingBufferError<'a, T> {}

    #[derive(Debug)]
    pub struct PoisonedLock<T>(pub PoisonError<T>);

    impl<'a, T> From<PoisonError<MutexGuard<'a, VecDeque<T>>>> for RingBufferError<'a, T> {
        fn from(err: PoisonError<MutexGuard<'a, VecDeque<T>>>) -> Self {
            RingBufferError::PoisonedLock(err)
        }
    }

    impl<T: fmt::Debug> fmt::Display for PoisonedLock<T> {
        fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(fmt, "internal ring buffer lock poisoned: {self:?}")
        }
    }
}
