#[allow(dead_code)]
pub(crate) struct Bus {
    tx: std::sync::mpsc::SyncSender<()>,
    rx: std::sync::mpsc::Receiver<()>,
}

#[allow(dead_code)]
impl Bus {
    pub fn new(size: usize) -> Self {
        let (tx, rx) = std::sync::mpsc::sync_channel(size);
        Self { tx, rx }
    }

    pub fn sender(&self) -> std::sync::mpsc::SyncSender<()> {
        self.tx.clone()
    }
}
