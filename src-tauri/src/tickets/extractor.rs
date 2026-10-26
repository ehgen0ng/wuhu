use std::{
    io::{Read, Write},
    path::Path,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use super::steam_client::{self, ExtractedTickets};

const EXTRACTOR_ARG: &str = "--wuhu-ticket-extractor";
const RESULT_PREFIX: &str = "WUHU_TICKET_RESULT:";
const EXTRACTOR_TIMEOUT: Duration = Duration::from_secs(45);

// Steam 以调用进程跟踪游戏状态；主界面不能承载目标游戏的 Steam 会话。
pub(super) fn extract(steam_root: &Path, app_id: u32) -> Result<ExtractedTickets, String> {
    let executable =
        std::env::current_exe().map_err(|err| format!("获取票据提取程序路径失败：{err}"))?;
    let mut command = Command::new(executable);
    command
        .arg(EXTRACTOR_ARG)
        .arg(steam_root)
        .arg(app_id.to_string())
        .env("SteamAppId", app_id.to_string())
        .env("SteamGameId", app_id.to_string());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    run_extractor(&mut command, EXTRACTOR_TIMEOUT)
}

struct ExtractorProcess(Child);

impl Drop for ExtractorProcess {
    fn drop(&mut self) {
        // 任何提前返回都必须回收进程，之后调用方才能恢复目标 Lua。
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn run_extractor(command: &mut Command, timeout: Duration) -> Result<ExtractedTickets, String> {
    let mut child = ExtractorProcess(
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|err| format!("启动票据提取进程失败：{err}"))?,
    );
    let mut stdout = child.0.stdout.take().expect("stdout is piped");
    // Steam 库也可能写 stdout，持续读取以免管道满后阻塞退出。
    let output = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).map(|_| bytes)
    });
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child
            .0
            .try_wait()
            .map_err(|err| format!("等待票据提取进程失败：{err}"))?
        {
            break status;
        }
        if started.elapsed() >= timeout {
            return Err("票据提取超时，已结束提取进程，请检查 Steam 状态后重试".to_string());
        }
        thread::sleep(Duration::from_millis(50));
    };
    let bytes = output
        .join()
        .map_err(|_| "读取票据提取结果失败".to_string())?
        .map_err(|err| format!("读取票据提取结果失败：{err}"))?;
    if !status.success() {
        return Err(format!("票据提取进程异常退出：{status}"));
    }
    parse_result(&bytes)
}

fn parse_result(bytes: &[u8]) -> Result<ExtractedTickets, String> {
    let text = String::from_utf8_lossy(bytes);
    let result = text
        .lines()
        .rev()
        .find_map(|line| line.strip_prefix(RESULT_PREFIX))
        .ok_or_else(|| "票据提取进程未返回结果".to_string())?;
    serde_json::from_str::<Result<ExtractedTickets, String>>(result)
        .map_err(|err| format!("解析票据提取结果失败：{err}"))?
}

// 在 Tauri 初始化前分流；辅助进程不创建窗口，也不读写应用设置。
pub(crate) fn run_if_requested() -> Option<i32> {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new(EXTRACTOR_ARG)) {
        return None;
    }
    let result = (|| {
        let steam_root = args.next().ok_or_else(|| "缺少 Steam 路径".to_string())?;
        let app_id = args
            .next()
            .and_then(|value| value.to_str().and_then(|value| value.parse::<u32>().ok()))
            .filter(|value| *value != 0)
            .ok_or_else(|| "AppID 无效".to_string())?;
        steam_client::extract(Path::new(&steam_root), app_id)
    })();
    // extract 返回前已释放用户、管道和动态库；这里只输出父进程读取的结果。
    let Ok(json) = serde_json::to_string(&result) else {
        return Some(1);
    };
    let mut stdout = std::io::stdout().lock();
    Some(
        if writeln!(stdout, "\n{RESULT_PREFIX}{json}")
            .and_then(|_| stdout.flush())
            .is_ok()
        {
            0
        } else {
            1
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_ignores_steam_logs_and_preserves_connection_errors() {
        let result = parse_result(b"Steam startup log\nWUHU_TICKET_RESULT:{\"Ok\":{\"app_ticket\":[1,2],\"e_ticket\":null}}\nSteam shutdown log\n").unwrap();
        assert_eq!(result.app_ticket, Some(vec![1, 2]));
        assert!(result.e_ticket.is_none());
        assert_eq!(
            parse_result(b"WUHU_TICKET_RESULT:{\"Err\":\"connection failed\"}\n").unwrap_err(),
            "connection failed"
        );
        assert!(parse_result(b"Steam log without result").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn process_finishes_before_returning_result() {
        let mut command = Command::new("/bin/sh");
        command.args([
            "-c",
            "printf '\\nWUHU_TICKET_RESULT:{\"Ok\":{\"app_ticket\":[7],\"e_ticket\":null}}\\n'; sleep 0.15",
        ]);
        let started = Instant::now();
        assert_eq!(
            run_extractor(&mut command, Duration::from_secs(2))
                .unwrap()
                .app_ticket,
            Some(vec![7])
        );
        assert!(started.elapsed() >= Duration::from_millis(100));
    }

    #[cfg(unix)]
    #[test]
    fn stalled_process_is_killed_and_reaped() {
        let mut command = Command::new("/bin/sleep");
        command.arg("30");
        let started = Instant::now();
        assert!(run_extractor(&mut command, Duration::from_millis(100))
            .unwrap_err()
            .contains("超时"));
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}
