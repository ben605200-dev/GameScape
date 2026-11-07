//! System sampling for desktop widgets. Read-only, entirely local.

use std::sync::Mutex;

use serde::Serialize;
use windows::Win32::Foundation::FILETIME;
use windows::Win32::System::SystemInformation::{
    GetTickCount64, GlobalMemoryStatusEx, MEMORYSTATUSEX,
};
use windows::Win32::System::Threading::GetSystemTimes;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemStats {
    /// CPU busy percentage since the previous sample (0 on the first call).
    pub cpu_percent: f32,
    pub memory_percent: f32,
    pub memory_used_mb: u64,
    pub memory_total_mb: u64,
    pub uptime_seconds: u64,
}

fn filetime_value(time: FILETIME) -> u64 {
    ((time.dwHighDateTime as u64) << 32) | time.dwLowDateTime as u64
}

/// (idle, kernel+user) of the previous sample, for the CPU delta.
static LAST_SAMPLE: Mutex<Option<(u64, u64)>> = Mutex::new(None);

pub fn sample() -> Result<SystemStats, String> {
    let mut memory = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    unsafe { GlobalMemoryStatusEx(&mut memory) }
        .map_err(|error| format!("cannot read memory status: {error}"))?;

    let mut idle = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    unsafe { GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)) }
        .map_err(|error| format!("cannot read system times: {error}"))?;

    let idle_value = filetime_value(idle);
    // Kernel time includes idle time; total = kernel + user.
    let total_value = filetime_value(kernel) + filetime_value(user);

    let cpu_percent = {
        let mut last = LAST_SAMPLE
            .lock()
            .map_err(|e| format!("stats lock poisoned: {e}"))?;
        let percent = match *last {
            Some((previous_idle, previous_total)) => {
                let idle_delta = idle_value.saturating_sub(previous_idle);
                let total_delta = total_value.saturating_sub(previous_total);
                if total_delta > 0 {
                    (100.0 * (total_delta.saturating_sub(idle_delta)) as f64 / total_delta as f64)
                        as f32
                } else {
                    0.0
                }
            }
            None => 0.0,
        };
        *last = Some((idle_value, total_value));
        percent
    };

    let used = memory.ullTotalPhys.saturating_sub(memory.ullAvailPhys);
    Ok(SystemStats {
        cpu_percent,
        memory_percent: memory.dwMemoryLoad as f32,
        memory_used_mb: used / (1024 * 1024),
        memory_total_mb: memory.ullTotalPhys / (1024 * 1024),
        uptime_seconds: unsafe { GetTickCount64() } / 1000,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_stats_are_sane() {
        sample().expect("first sample");
        std::thread::sleep(std::time::Duration::from_millis(250));
        let second = sample().expect("second sample");
        assert!((0.0..=100.0).contains(&second.cpu_percent));
        assert!((0.0..=100.0).contains(&second.memory_percent));
        assert!(second.memory_total_mb > 0);
        assert!(second.uptime_seconds > 0);
        assert!(second.memory_used_mb <= second.memory_total_mb);
    }
}
