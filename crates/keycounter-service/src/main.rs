use anyhow::{Context, Result};
use keycounter_common::{append_record, decode_snapshot, Config, NamedPipeServer, KEY_COUNT};
use std::{path::PathBuf, sync::{Arc, Mutex}};
use windows_service::{define_windows_service, service::{ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus, ServiceType}, service_control_handler::{self, ServiceControlHandlerResult}, service_dispatcher};

const SERVICE_NAME: &str = "KeyCounterService";
const ERROR_LOG: &str = r"C:\ProgramData\KeyCounter\service-error.log";

fn log_error(message: &str) {
    use std::io::Write;
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(ERROR_LOG) {
        let _ = writeln!(file, "{}: {message}", chrono_like_timestamp());
    }
}

fn chrono_like_timestamp() -> String {
    format!("{:?}", std::time::SystemTime::now())
}

pub fn main() -> Result<()> {
    service_dispatcher::start(SERVICE_NAME, ffi_service_main).context("failed to start service dispatcher")?;
    Ok(())
}

define_windows_service!(ffi_service_main, service_main);

fn service_main(_arguments: Vec<std::ffi::OsString>) {
    if let Err(e) = run_service() {
        log_error(&format!("{e:#}"));
        eprintln!("KeyCounterService failed: {e:#}");
    }
}

fn run_service() -> Result<()> {
    let config_path = std::env::var_os("KEYCOUNTER_CONFIG").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\ProgramData\KeyCounter\config.yaml"));
    let config = Config::load(&config_path)?;
    std::fs::create_dir_all(&config.storage.directory)?;

    let stopped = Arc::new(Mutex::new(false));
    let stopped_for_handler = stopped.clone();

    let status_handle = service_control_handler::register(SERVICE_NAME, move |event| {
        match event {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                *stopped_for_handler.lock().unwrap() = true;
                ServiceControlHandlerResult::NoError
            }
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    })?;

    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: std::time::Duration::default(),
        process_id: None,
    })?;

    let pipe_name = config.ipc.pipe_name.clone();
    let storage = config.storage.directory.clone();
    let output = storage.join("keyboard.kbd");

    while !*stopped.lock().unwrap() {
        let server = NamedPipeServer::create(&pipe_name)
            .map_err(|e| anyhow::anyhow!("failed to create named pipe {pipe_name}: {e:#}"))?;
        server.wait_for_client()?;

        let mut packet = vec![0u8; 8 + KEY_COUNT * 4];
        if server.read_exact(&mut packet).is_ok() {
            if let Ok(counts) = decode_snapshot(&packet) {
                if append_record(&output, &counts).is_ok() {
                    let _ = server.write_all(b"ACK1");
                }
            }
        }
        server.disconnect();
    }

    status_handle.set_service_status(ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: std::time::Duration::default(),
        process_id: None,
    })?;

    Ok(())
}
