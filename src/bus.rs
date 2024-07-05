use crossbeam::channel::{self, Receiver, Sender};

pub type BusChannel<T> = (Sender<T>, Receiver<T>);

#[allow(dead_code)]
pub(crate) struct Bus<T> {
    pub tx: channel::Sender<T>,
    pub rx: channel::Receiver<T>,
}

#[allow(dead_code)]
impl<T> Bus<T> {
    pub fn new(size: usize) -> Self {
        let (tx, rx) = channel::bounded(size);
        Self { tx, rx }
    }

    pub fn clone(&self) -> BusChannel<T> {
        (self.tx.clone(), self.rx.clone())
    }
}
