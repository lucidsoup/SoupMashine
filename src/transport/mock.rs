//! An in-memory [`Transport`] for tests and for running headless without a
//! physical Maschine attached.

use super::Transport;
use std::collections::VecDeque;
use std::io;

/// Records everything written and replays a queue of injected input reports.
#[derive(Default)]
pub struct MockTransport {
    /// Pending input reports to hand back from `read_input`, FIFO.
    pub input_queue: VecDeque<Vec<u8>>,
    /// Every LED report written, in order.
    pub outputs: Vec<Vec<u8>>,
    /// Every display chunk written, in order.
    pub display_chunks: Vec<Vec<u8>>,
}

impl MockTransport {
    pub fn new() -> Self {
        Self::default()
    }

    /// Queue a raw input report to be returned by a later `read_input`.
    pub fn inject(&mut self, report: Vec<u8>) {
        self.input_queue.push_back(report);
    }
}

impl Transport for MockTransport {
    fn read_input(&mut self, buf: &mut [u8], _timeout_ms: u32) -> io::Result<usize> {
        match self.input_queue.pop_front() {
            Some(report) => {
                let n = report.len().min(buf.len());
                buf[..n].copy_from_slice(&report[..n]);
                Ok(n)
            }
            None => Ok(0), // timeout / nothing pending
        }
    }

    fn write_output(&mut self, report: &[u8]) -> io::Result<()> {
        self.outputs.push(report.to_vec());
        Ok(())
    }

    fn write_display(&mut self, chunk: &[u8]) -> io::Result<()> {
        self.display_chunks.push(chunk.to_vec());
        Ok(())
    }
}
