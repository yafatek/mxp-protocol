use std::fs::File;
use std::io::{self, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// Thread-safe wrapper around a PCAP writer.
#[derive(Clone)]
pub struct PcapRecorder {
    inner: Arc<Mutex<PcapWriter>>,
}

impl PcapRecorder {
    /// Create a recorder that writes to the provided path, truncating any existing file.
    pub fn create(path: &Path) -> io::Result<Self> {
        let writer = PcapWriter::new(path)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(writer)),
        })
    }

    /// Record a packet with the current system timestamp.
    pub fn record(&self, packet: &[u8]) -> io::Result<()> {
        let timestamp = SystemTime::now();
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| io::Error::other("pcap recorder poisoned"))?;
        guard.write_packet(timestamp, packet)
    }
}

impl std::fmt::Debug for PcapRecorder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PcapRecorder").finish_non_exhaustive()
    }
}

struct PcapWriter {
    file: File,
}

impl PcapWriter {
    fn new(path: &Path) -> io::Result<Self> {
        let mut file = File::create(path)?;
        write_global_header(&mut file)?;
        Ok(Self { file })
    }

    fn write_packet(&mut self, timestamp: SystemTime, data: &[u8]) -> io::Result<()> {
        let (sec, usec) = micros(timestamp);
        let length = data.len().min(u32::MAX as usize) as u32;
        let mut header = [0u8; 16];
        header[0..4].copy_from_slice(&sec.to_le_bytes());
        header[4..8].copy_from_slice(&usec.to_le_bytes());
        header[8..12].copy_from_slice(&length.to_le_bytes());
        header[12..16].copy_from_slice(&length.to_le_bytes());

        self.file.write_all(&header)?;
        self.file.write_all(&data[..length as usize])?;
        self.file.flush()?;
        Ok(())
    }
}

const PCAP_MAGIC: u32 = 0xa1b2_c3d4;
const PCAP_VERSION_MAJOR: u16 = 2;
const PCAP_VERSION_MINOR: u16 = 4;
const PCAP_THISZONE: i32 = 0;
const PCAP_SIGFIGS: u32 = 0;
const PCAP_SNAPLEN: u32 = 65_535;
const PCAP_NETWORK: u32 = 101; // LINKTYPE_RAW

fn write_global_header(file: &mut File) -> io::Result<()> {
    let mut header = [0u8; 24];
    header[0..4].copy_from_slice(&PCAP_MAGIC.to_le_bytes());
    header[4..6].copy_from_slice(&PCAP_VERSION_MAJOR.to_le_bytes());
    header[6..8].copy_from_slice(&PCAP_VERSION_MINOR.to_le_bytes());
    header[8..12].copy_from_slice(&PCAP_THISZONE.to_le_bytes());
    header[12..16].copy_from_slice(&PCAP_SIGFIGS.to_le_bytes());
    header[16..20].copy_from_slice(&PCAP_SNAPLEN.to_le_bytes());
    header[20..24].copy_from_slice(&PCAP_NETWORK.to_le_bytes());
    file.write_all(&header)
}

fn micros(timestamp: SystemTime) -> (u32, u32) {
    let duration = timestamp.duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = duration.as_secs().min(u64::from(u32::MAX)) as u32;
    let micros = duration.subsec_micros();
    (secs, micros)
}
