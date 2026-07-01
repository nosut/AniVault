use std::time::{SystemTime, UNIX_EPOCH};
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::ProcessStatus::K32EnumProcesses;
use windows::Win32::System::ProcessStatus::K32GetModuleFileNameExW;
use windows::Win32::System::Threading::OpenProcess;
use windows::Win32::System::Threading::PROCESS_QUERY_INFORMATION;
use windows::Win32::System::Threading::PROCESS_VM_READ;

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

pub fn scan_active_players(config: &ScannerConfig) -> Vec<ScanResult> {
    let known = known_process_names(config);
    if known.is_empty() {
        return vec![];
    }

    let mut results: Vec<ScanResult> = Vec::new();
    let mut pids = vec![0u32; 1024];
    let mut bytes_returned: u32 = 0;

    // SAFETY: pids is sized correctly; error means no permission/view, skip.
    let _ = unsafe {
        K32EnumProcesses(
            pids.as_mut_ptr(),
            (pids.len() * std::mem::size_of::<u32>()) as u32,
            &mut bytes_returned,
        )
    };

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
                    results.push(ScanResult {
                        player_name: player.process_name.clone(),
                        file_path: Some(name),
                        window_title: None,
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

    results
}
