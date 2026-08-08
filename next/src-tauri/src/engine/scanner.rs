use std::time::{SystemTime, UNIX_EPOCH};
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Foundation::HWND;
use windows::Win32::Foundation::LPARAM;
use windows::Win32::System::ProcessStatus::K32EnumProcesses;
use windows::Win32::System::ProcessStatus::K32GetModuleFileNameExW;
use windows::Win32::System::Threading::OpenProcess;
use windows::Win32::System::Threading::PROCESS_QUERY_INFORMATION;
use windows::Win32::System::Threading::PROCESS_VM_READ;
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
    let mut pids = vec![0u32; 1024];
    let mut bytes_returned: u32 = 0;

    // SAFETY: pids is sized correctly; error means no permission/view, skip.
    let enumerated = unsafe {
        K32EnumProcesses(
            pids.as_mut_ptr(),
            (pids.len() * std::mem::size_of::<u32>()) as u32,
            &mut bytes_returned,
        )
    }
    .as_bool();

    let count = (bytes_returned as usize) / std::mem::size_of::<u32>();

    for &pid in &pids[..count.min(pids.len())] {
        if pid == 0 {
            continue;
        }

        // SAFETY: OpenProcess may fail for system processes; skip on failure.
        let handle = unsafe {
            OpenProcess(
                PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
                false,
                pid,
            )
        };
        let Ok(handle) = handle else {
            continue;
        };

        let mut exe_path = vec![0u16; 260];
        // SAFETY: handle is a valid process handle; buffer is correctly sized.
        let len = unsafe {
            K32GetModuleFileNameExW(
                Some(handle),
                None,
                &mut exe_path,
            )
        };
        if len != 0 {
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
