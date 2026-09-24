// WorkBuddy2API — Tauri 原生 Mac 客户端
// 启动时拉起 admin.server（若未运行），然后轮询 /health 加载管理后台
#![cfg_attr(not(debug_assertions), deny(warnings))]

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;
use tauri::{Manager, WindowEvent};

const PORT: u16 = 8787;

/// 定位仓库根：Contents/.repo_root 标记 → ~/dev/codebuddy2openai
fn resolve_repo() -> Option<PathBuf> {
    let marker = std::env::current_exe()
        .ok()?
        .parent()? // MacOS/
        .parent()? // Contents/
        .join(".repo_root");
    if let Ok(p) = std::fs::read_to_string(&marker) {
        let p = PathBuf::from(p.trim());
        if p.join(".venv/bin/python").exists() {
            return Some(p);
        }
    }
    let home = std::env::var("HOME").ok()?;
    let p = PathBuf::from(home).join("dev/codebuddy2openai");
    if p.join(".venv/bin/python").exists() {
        return Some(p);
    }
    None
}

/// 读取或生成 data/management/.keys，返回 (ADMIN_KEY, CLIENT_KEY)
fn ensure_keys(repo: &PathBuf) -> (String, String) {
    let dir = repo.join("data/management");
    let _ = std::fs::create_dir_all(&dir);
    let keys_file = dir.join(".keys");
    if let Ok(content) = std::fs::read_to_string(&keys_file) {
        let mut ak = String::new();
        let mut ck = String::new();
        for line in content.lines() {
            if let Some(v) = line.strip_prefix("ADMIN_KEY=") {
                ak = v.to_string();
            }
            if let Some(v) = line.strip_prefix("CLIENT_KEY=") {
                ck = v.to_string();
            }
        }
        if !ak.is_empty() && !ck.is_empty() && ak.len() >= 20 {
            return (ak, ck);
        }
        // 密钥无效（如历史上写入了短密钥），落下来重新生成并覆盖
    }
    // 默认 20 字符（admin.server 硬要求），客户端 Key 随机
    let ak = "admin-admin-admin-admin".to_string();
    let ck = format!("sk-{}", uuid_like());
    let _ = std::fs::write(&keys_file, format!("ADMIN_KEY={}\nCLIENT_KEY={}\n", ak, ck));
    (ak, ck)
}

/// 无外部 crate 的简易随机 hex（32 字符）
fn uuid_like() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let pid = std::process::id();
    let mut out = String::new();
    let mut seed = nanos ^ ((pid as u128) << 64);
    // ponytail: xorshift 非加密随机，够用于本地 API key
    for _ in 0..16 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        out.push_str(&format!("{:04x}", (seed & 0xffff) as u16));
    }
    out
}

/// 若 8787 未在跑，后台拉起 admin.server
fn ensure_server(repo: &PathBuf) {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(1))
        .build()
        .expect("client");
    if client
        .get(format!("http://127.0.0.1:{}/health", PORT))
        .send()
        .map(|r| r.status().is_success())
        .unwrap_or(false)
    {
        return; // 已在跑
    }
    let (ak, ck) = ensure_keys(repo);
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(repo.join("data/management/service.log"))
        .ok();
    let _ = Command::new(repo.join(".venv/bin/python"))
        .arg("-m")
        .arg("admin.server")
        .current_dir(repo)
        .env("MANAGEMENT_DATA_DIR", repo.join("data/management"))
        .env(
            "CODEBUDDY_AUTH_DIR",
            format!(
                "{}/Library/Application Support/CodeBuddyExtension/Data/Public/auth",
                std::env::var("HOME").unwrap_or_default()
            ),
        )
        .env("CODEBUDDY2OPENAI_KEY", ck)
        .env("ADMIN_KEY", ak)
        .stdout(Stdio::from_or_null(log))
        .spawn();
}

trait StdioFromOrNull {
    fn from_or_null(f: Option<std::fs::File>) -> Stdio;
}
impl StdioFromOrNull for Stdio {
    fn from_or_null(f: Option<std::fs::File>) -> Stdio {
        match f {
            Some(f) => Stdio::from(f),
            None => Stdio::null(),
        }
    }
}

#[tauri::command]
async fn wait_for_health() -> Result<bool, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .map_err(|e| e.to_string())?;
    for _ in 0..60 {
        match client.get(format!("http://127.0.0.1:{}/health", PORT)).send().await {
            Ok(resp) if resp.status().is_success() => return Ok(true),
            Ok(_) | Err(_) => tokio::time::sleep(Duration::from_millis(500)).await,
        }
    }
    Ok(false)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![wait_for_health])
        .setup(|app| {
            let window = app.get_webview_window("main").expect("no main window");
            let app_handle = app.app_handle().clone();
            window.on_window_event(move |event| {
                if let WindowEvent::CloseRequested { .. } = event {
                    // 关闭主窗口 = 退出应用（服务保持后台运行）
                    app_handle.exit(0);
                }
            });
            // 拉起服务（若未运行）
            if let Some(repo) = resolve_repo() {
                ensure_server(&repo);
            }
            tauri::async_runtime::spawn(async move {
                if wait_for_health().await.unwrap_or(false) {
                    let _ = window.eval("location.href='http://127.0.0.1:8787/admin/'");
                }
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn main() {
    run();
}