use std::time::{SystemTime, UNIX_EPOCH};
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Foundation::HWND;
use windows::Win32::Foundation::LPARAM;
use windows::core::PWSTR;
use windows::Win32::System::ProcessStatus::K32EnumProcesses;
use windows::Win32::System::Threading::OpenProcess;
use windows::Win32::System::Threading::QueryFullProcessImageNameW;
use windows::Win32::System::Threading::PROCESS_NAME_WIN32;
use windows::Win32::System::Threading::PROCESS_QUERY_LIMITED_INFORMATION;
use windows::Win32::UI::WindowsAndMessaging::EnumWindows;
use windows::Win32::UI::WindowsAndMessaging::GetWindowTextLengthW;
use windows::Win32::UI::WindowsAndMessaging::GetWindowTextW;
use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

#[derive(Debug, Clone)]
pub struct PlayerDef {
    pub process_name: String,
    pub window_title_hint: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ScannerConfig {
    pub known_players: Vec<PlayerDef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanResult {
    pub player_name: String,
    pub file_path: Option<String>,
    pub window_title: Option<String>,
    pub detected_at_unix: i64,
}

/// The outcome of one process scan.
///
/// `enumerated` is what lets a caller read an empty `players` as "the player
/// closed" rather than "we could not tell". Without it a failed enumeration
/// looks exactly like a player that exited, and the tracker would end a live
/// session on a transient failure.
#[derive(Debug, Clone, Default)]
pub struct PlayerScan {
    /// Known media players found running.
    pub players: Vec<ScanResult>,
    /// Whether the process list was actually read this scan.
    pub enumerated: bool,
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn known_process_names(config: &ScannerConfig) -> Vec<String> {
    config
        .known_players
        .iter()
        .map(|p| p.process_name.to_lowercase())
        .collect()
}

struct EnumState {
    target_pid: u32,
    title: Option<String>,
}

unsafe extern "system" fn enum_window_callback(
    hwnd: HWND,
    lparam: LPARAM,
) -> windows::core::BOOL {
    let state = &mut *(lparam.0 as *mut EnumState);
    let mut pid: u32 = 0;
    GetWindowThreadProcessId(hwnd, Some(&mut pid as *mut u32));
    if pid == state.target_pid {
        let len = GetWindowTextLengthW(hwnd);
        if len > 0 {
            let mut buf = vec![0u16; (len + 1) as usize];
            let actual = GetWindowTextW(hwnd, &mut buf);
            if actual > 0 {
                let title = String::from_utf16_lossy(&buf[..actual as usize]);
                if !title.is_empty() {
                    state.title = Some(title);
                    return windows::core::BOOL::from(false);
                }
            }
        }
    }
    windows::core::BOOL::from(true)
}

unsafe fn get_process_window_title(target_pid: u32) -> Option<String> {
    let mut state = EnumState {
        target_pid,
        title: None,
    };
    let state_ptr = &mut state as *mut EnumState;
    let _ = EnumWindows(Some(enum_window_callback), LPARAM(state_ptr as isize));
    state.title
}

/// Enumerate process ids with `enum_fn`, which fills the buffer and returns the
/// bytes written (or `None` on failure) the way `EnumProcesses` does. That API
/// cannot say how many processes exist: a completely full buffer may have been
/// truncated, so grow it and ask again until there is room to spare.
pub fn enumerate_pids_with<F>(mut enum_fn: F) -> (Vec<u32>, bool)
where
    F: FnMut(&mut [u32]) -> Option<u32>,
{
    const MAX_SLOTS: usize = 1 << 20;
    let mut slots = 1024usize;
    loop {
        let mut pids = vec![0u32; slots];
        let Some(bytes) = enum_fn(&mut pids) else {
            return (Vec::new(), false);
        };
        let count = (bytes as usize / std::mem::size_of::<u32>()).min(slots);
        if count < slots || slots >= MAX_SLOTS {
            pids.truncate(count);
            return (pids, true);
        }
        slots *= 2;
    }
}

pub fn scan_active_players(config: &ScannerConfig) -> PlayerScan {
    let known = known_process_names(config);
    if known.is_empty() {
        // Nothing is trackable, so "no players running" is trivially true and
        // safe for a caller to act on.
        return PlayerScan {
            players: vec![],
            enumerated: true,
        };
    }

    let mut results: Vec<ScanResult> = Vec::new();
    let (pids, enumerated) = enumerate_pids_with(|buf| {
        let mut bytes_returned: u32 = 0;
        // SAFETY: the buffer pointer and its byte size describe the same slice.
        let ok = unsafe {
            K32EnumProcesses(
                buf.as_mut_ptr(),
                std::mem::size_of_val(buf) as u32,
                &mut bytes_returned,
            )
        }
        .as_bool();
        ok.then_some(bytes_returned)
    });

    // Reused for every process: the longest possible Win32 path.
    let mut exe_path = vec![0u16; 32_768];
    for &pid in &pids {
        if pid == 0 {
            continue;
        }

        // Limited query rights are enough for the image name and, unlike
        // PROCESS_VM_READ, are granted for elevated processes too, so a player
        // run as administrator is still seen.
        // SAFETY: OpenProcess may fail for system processes; skip on failure.
        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) };
        let Ok(handle) = handle else {
            continue;
        };

        let mut len = exe_path.len() as u32;
        // SAFETY: handle is a valid process handle; `len` holds the buffer size in chars.
        let queried = unsafe {
            QueryFullProcessImageNameW(
                handle,
                PROCESS_NAME_WIN32,
                PWSTR(exe_path.as_mut_ptr()),
                &mut len,
            )
        };
        if queried.is_ok() && len != 0 {
            let len = (len as usize).min(exe_path.len());
            let name = String::from_utf16_lossy(&exe_path[..len]);
            let name_lower = name.to_lowercase();
            for player in &config.known_players {
                let process_lower = player.process_name.to_lowercase();
                if name_lower.ends_with(&format!("\\{}", process_lower))
                    || name_lower == process_lower
                {
                    // SAFETY: get_process_window_title uses EnumWindows with a valid PID.
                    let window_title = unsafe { get_process_window_title(pid) };

                    results.push(ScanResult {
                        player_name: player.process_name.clone(),
                        file_path: window_title.clone().or(Some(name)),
                        window_title,
                        detected_at_unix: unix_now(),
                    });
                    break;
                }
            }
        }

        // SAFETY: CloseHandle on a valid handle is always safe.
        unsafe {
            let _ = CloseHandle(handle);
        }
    }

    PlayerScan {
        players: results,
        enumerated,
    }
}
