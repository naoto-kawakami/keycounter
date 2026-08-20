#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::{Context, Result};
use keycounter_common::{encode_snapshot, Config, NamedPipeClient, KEY_COUNT};
use std::{path::PathBuf, sync::atomic::{AtomicBool, Ordering}, thread, time::Duration};
use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{CallNextHookEx, DispatchMessageW, GetMessageW, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, HHOOK, HC_ACTION, KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL, WM_KEYDOWN, WM_SYSKEYDOWN};

static mut COUNTS: [u32; KEY_COUNT] = [0; KEY_COUNT];
static STOP: AtomicBool = AtomicBool::new(false);

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && (wparam.0 == WM_KEYDOWN as usize || wparam.0 == WM_SYSKEYDOWN as usize) {
        let info = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        let vk = info.vkCode as usize;
        if vk < KEY_COUNT {
            COUNTS[vk] = COUNTS[vk].saturating_add(1);
        }
    }
    CallNextHookEx(Some(HHOOK::default()), code, wparam, lparam)
}

fn snapshot_and_reset() -> [u32; KEY_COUNT] {
    unsafe {
        let snapshot = COUNTS;
        COUNTS = [0; KEY_COUNT];
        snapshot
    }
}

fn send_periodically(config: Config) {
    let interval = Duration::from_secs(config.logging.interval_minutes * 60);
    while !STOP.load(Ordering::Relaxed) {
        thread::sleep(interval);
        if STOP.load(Ordering::Relaxed) { break; }

        let snapshot = snapshot_and_reset();
        if snapshot.iter().all(|v| *v == 0) { continue; }

        let payload = encode_snapshot(&snapshot);
        match NamedPipeClient::connect(&config.ipc.pipe_name, 3000)
            .and_then(|pipe| {
                pipe.write_all(&payload)?;
                let mut ack = [0u8; 4];
                pipe.read_exact(&mut ack)?;
                if &ack != b"ACK1" { anyhow::bail!("service rejected snapshot"); }
                Ok(())
            })
        {
            Ok(()) => {}
            Err(_) => {
                // Preserve the counts if the service was unavailable.
                unsafe {
                    for i in 0..KEY_COUNT {
                        COUNTS[i] = COUNTS[i].saturating_add(snapshot[i]);
                    }
                }
            }
        }
    }
}

fn config_path() -> PathBuf {
    std::env::var_os("KEYCOUNTER_CONFIG").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("config.yaml"))
}

fn main() -> Result<()> {
    let config = Config::load(config_path())?;
    let config_for_thread = config.clone();
    thread::spawn(move || send_periodically(config_for_thread));

    unsafe {
        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), Some(HINSTANCE::default()), 0)
            .context("failed to install WH_KEYBOARD_LL")?;

        let mut msg = MSG::default();
        while !STOP.load(Ordering::Relaxed) && GetMessageW(&mut msg, None, 0, 0).0 > 0 {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        UnhookWindowsHookEx(hook)?;
    }
    Ok(())
}
