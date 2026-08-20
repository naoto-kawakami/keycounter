use anyhow::{Context, Result};
use serde::Deserialize;
use std::{fs, path::{Path, PathBuf}};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows::Win32::Storage::FileSystem::{CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ, FILE_GENERIC_WRITE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, PIPE_ACCESS_DUPLEX, ReadFile, WriteFile};
use windows::Win32::System::Pipes::{ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, WaitNamedPipeW, PIPE_READMODE_MESSAGE, PIPE_TYPE_MESSAGE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT};

pub const KEY_COUNT: usize = 256;
pub const PROTOCOL_MAGIC: [u8; 4] = *b"KHM1";
pub const FILE_MAGIC: [u8; 4] = *b"KBD1";
pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub logging: LoggingConfig,
    pub storage: StorageConfig,
    pub keyboard: KeyboardConfig,
    pub ipc: IpcConfig,
    pub privacy: PrivacyConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingConfig {
    pub interval_minutes: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StorageConfig {
    pub directory: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KeyboardConfig {
    pub layout: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IpcConfig {
    pub pipe_name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PrivacyConfig {
    pub record_key_sequence: bool,
    pub record_characters: bool,
    pub record_timestamps: bool,
}

impl Config {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let text = fs::read_to_string(path.as_ref())
            .with_context(|| format!("failed to read {}", path.as_ref().display()))?;
        let cfg: Self = serde_yaml::from_str(&text).context("invalid YAML configuration")?;
        if cfg.logging.interval_minutes == 0 {
            anyhow::bail!("logging.interval_minutes must be > 0");
        }
        if cfg.privacy.record_key_sequence || cfg.privacy.record_characters || cfg.privacy.record_timestamps {
            anyhow::bail!("privacy options that record sequence, characters, or timestamps must remain false");
        }
        Ok(cfg)
    }
}

pub fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub struct NamedPipeServer {
    handle: HANDLE,
}

impl NamedPipeServer {
    pub fn create(name: &str) -> Result<Self> {
        let name = wide(name);
        unsafe {
            let h = CreateNamedPipeW(
                PCWSTR(name.as_ptr()),
                PIPE_ACCESS_DUPLEX,
                PIPE_TYPE_MESSAGE | PIPE_READMODE_MESSAGE | PIPE_WAIT,
                PIPE_UNLIMITED_INSTANCES,
                4096,
                4096,
                0,
                None,
            );
            if h == INVALID_HANDLE_VALUE {
                return Err(windows::core::Error::from_win32().into());
            }
            Ok(Self { handle: h })
        }
    }

    pub fn wait_for_client(&self) -> Result<()> {
        unsafe {
            let ok = ConnectNamedPipe(self.handle, None);
            if ok.is_err() {
                let e = windows::core::Error::from_win32();
                // ERROR_PIPE_CONNECTED means the client connected between CreateNamedPipe and ConnectNamedPipe.
                if e.code().0 != 535 { // ERROR_PIPE_CONNECTED
                    return Err(e.into());
                }
            }
        }
        Ok(())
    }

    pub fn read_exact(&self, buf: &mut [u8]) -> Result<()> {
        let mut offset = 0usize;
        while offset < buf.len() {
            let mut read = 0u32;
            unsafe {
                ReadFile(self.handle, Some(&mut buf[offset..]), Some(&mut read), None)?;
            }
            if read == 0 { anyhow::bail!("named pipe closed"); }
            offset += read as usize;
        }
        Ok(())
    }

    pub fn write_all(&self, buf: &[u8]) -> Result<()> {
        let mut offset = 0usize;
        while offset < buf.len() {
            let mut written = 0u32;
            unsafe {
                WriteFile(self.handle, Some(&buf[offset..]), Some(&mut written), None)?;
            }
            if written == 0 { anyhow::bail!("named pipe write returned zero"); }
            offset += written as usize;
        }
        Ok(())
    }

    pub fn disconnect(&self) {
        unsafe { let _ = DisconnectNamedPipe(self.handle); }
    }
}

impl Drop for NamedPipeServer {
    fn drop(&mut self) {
        unsafe { let _ = CloseHandle(self.handle); }
    }
}

pub struct NamedPipeClient {
    handle: HANDLE,
}

impl NamedPipeClient {
    pub fn connect(name: &str, timeout_ms: u32) -> Result<Self> {
        let name_w = wide(name);
        unsafe {
            if !WaitNamedPipeW(PCWSTR(name_w.as_ptr()), timeout_ms).as_bool() {
                return Err(windows::core::Error::from_win32().into());
            }
            let h = CreateFileW(
                PCWSTR(name_w.as_ptr()),
                (FILE_GENERIC_READ | FILE_GENERIC_WRITE).0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                None,
            )?;
            if h == INVALID_HANDLE_VALUE { anyhow::bail!("invalid named pipe handle"); }
            Ok(Self { handle: h })
        }
    }

    pub fn read_exact(&self, buf: &mut [u8]) -> Result<()> {
        let mut offset = 0usize;
        while offset < buf.len() {
            let mut read = 0u32;
            unsafe { ReadFile(self.handle, Some(&mut buf[offset..]), Some(&mut read), None)?; }
            if read == 0 { anyhow::bail!("named pipe closed"); }
            offset += read as usize;
        }
        Ok(())
    }

    pub fn write_all(&self, buf: &[u8]) -> Result<()> {
        let mut offset = 0usize;
        while offset < buf.len() {
            let mut written = 0u32;
            unsafe { WriteFile(self.handle, Some(&buf[offset..]), Some(&mut written), None)?; }
            if written == 0 { anyhow::bail!("named pipe write returned zero"); }
            offset += written as usize;
        }
        Ok(())
    }
}

impl Drop for NamedPipeClient {
    fn drop(&mut self) {
        unsafe { let _ = CloseHandle(self.handle); }
    }
}

pub fn encode_snapshot(counts: &[u32; KEY_COUNT]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + 4 + KEY_COUNT * 4);
    out.extend_from_slice(&PROTOCOL_MAGIC);
    out.extend_from_slice(&PROTOCOL_VERSION.to_le_bytes());
    for v in counts { out.extend_from_slice(&v.to_le_bytes()); }
    out
}

pub fn decode_snapshot(buf: &[u8]) -> Result<[u32; KEY_COUNT]> {
    if buf.len() != 4 + 4 + KEY_COUNT * 4 { anyhow::bail!("invalid snapshot length"); }
    if buf[0..4] != PROTOCOL_MAGIC { anyhow::bail!("invalid protocol magic"); }
    let version = u32::from_le_bytes(buf[4..8].try_into().unwrap());
    if version != PROTOCOL_VERSION { anyhow::bail!("unsupported protocol version"); }
    let mut counts = [0u32; KEY_COUNT];
    for i in 0..KEY_COUNT {
        let start = 8 + i * 4;
        counts[i] = u32::from_le_bytes(buf[start..start + 4].try_into().unwrap());
    }
    Ok(counts)
}

pub fn append_record(path: &Path, counts: &[u32; KEY_COUNT]) -> Result<()> {
    use std::io::Write;
    let new_file = !path.exists();
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    if new_file {
        f.write_all(&FILE_MAGIC)?;
        f.write_all(&1u32.to_le_bytes())?;
        f.write_all(&(KEY_COUNT as u32).to_le_bytes())?;
    }
    for v in counts { f.write_all(&v.to_le_bytes())?; }
    f.flush()?;
    Ok(())
}
