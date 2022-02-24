use std::{sync::{Arc, Mutex, MutexGuard}, ops::Deref, io};
use tracing::dispatcher::DefaultGuard;
use tracing_subscriber::{fmt::MakeWriter, util::SubscriberInitExt};

#[derive(Clone)]
pub struct MakeMemoryWriter {
    buffer: Arc<Mutex<Vec<u8>>>,
}

pub struct GuardWrapper<'a>(MutexGuard<'a, Vec<u8>>);

impl<'a> io::Write for GuardWrapper<'a> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

impl<'a> MakeWriter<'a> for MakeMemoryWriter {
    type Writer = GuardWrapper<'a>;
    fn make_writer(&'a self) -> Self::Writer {
        self.buffer.lock().map(GuardWrapper).expect("memory writer mutex was poisoned")
    }
}

impl MakeMemoryWriter {
    fn clear(&self) {
        self.buffer.lock().expect("memory writer lock was poisoned").clear();
    }
}

pub struct MemorySubscriber {
    writer: MakeMemoryWriter,
    guard: DefaultGuard,
}

impl MemorySubscriber {
    pub fn new(log_level: tracing::Level) -> Self {
        let writer = MakeMemoryWriter { buffer: Arc::new(Mutex::new(Vec::new())) };
        let subscriber = hoard::logging::get_subscriber()
            .with_writer(writer.clone())
            .with_max_level(log_level)
            .finish();
        let guard = subscriber.set_default();
        MemorySubscriber { writer, guard }
    }
    
    pub fn output(&'_ self) -> impl Deref<Target=Vec<u8>> + '_ {
        self.writer.buffer.lock().expect("memory writer lock was poisoned")
    }

    pub fn clear(&self) {
        self.writer.clear();
    }
}

impl Default for MemorySubscriber {
    fn default() -> Self {
        Self::new(tracing::Level::INFO)
    }
}