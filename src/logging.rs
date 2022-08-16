use std::{
    io,
    sync::{mpsc, Mutex},
};

use tracing::{debug, metadata::LevelFilter, subscriber::DefaultGuard};
use tracing_subscriber::prelude::*;

struct MpscWriter {
    sender: mpsc::Sender<Vec<u8>>,
}

impl MpscWriter {
    fn new(sender: mpsc::Sender<Vec<u8>>) -> Self {
        Self { sender }
    }
}

impl io::Write for MpscWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.sender
            .send(buf.to_vec())
            .map_err(|error| io::Error::new(io::ErrorKind::Other, error))?;

        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub struct Logger {
    rx: mpsc::Receiver<Vec<u8>>,
    _guard: DefaultGuard,
}

impl Logger {
    #[must_use = "log capturing works only until `Logger` is dropped"]
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        let tx = Mutex::new(tx);

        let subscriber = tracing_subscriber::fmt()
            .with_max_level(LevelFilter::TRACE)
            .without_time()
            .with_ansi(false);

        let writer =
            move || MpscWriter::new(tx.lock().expect("should be able to lock mutex").clone());

        #[cfg(not(target_family = "wasm"))]
        let subscriber = subscriber.with_writer(io::stderr.and(writer));
        #[cfg(target_family = "wasm")]
        let subscriber = subscriber.with_writer(writer);

        let subscriber = subscriber.finish();

        #[cfg(target_family = "wasm")]
        let subscriber = subscriber.with(tracing_wasm::WASMLayer::new(
            tracing_wasm::WASMLayerConfigBuilder::new()
                .set_console_config(tracing_wasm::ConsoleConfig::ReportWithoutConsoleColor)
                .set_max_level(tracing::Level::TRACE)
                .set_report_logs_in_timings(false)
                .build(),
        ));

        let guard = tracing::subscriber::set_default(subscriber);

        Self { rx, _guard: guard }
    }

    pub fn collect_log(&self) -> String {
        debug!("collecting log");

        let mut log = Vec::new();
        while let Ok(mut message) = self.rx.try_recv() {
            log.append(&mut message);
        }

        String::from_utf8_lossy(&log).into_owned()
    }
}

pub fn separator() {
    debug!("----------------------------------------------------------------------");
}
