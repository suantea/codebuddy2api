// WorkBuddy2API — Tauri 原生 Mac 客户端
// 启动后轮询 /health，服务就绪即加载管理后台页面
#![cfg_attr(not(debug_assertions), deny(warnings))]

use std::time::Duration;
use tauri::{Manager, WindowEvent};

#[tauri::command]
async fn wait_for_health() -> Result<bool, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .map_err(|e| e.to_string())?;
    for _ in 0..60 {
        match client.get("http://127.0.0.1:8787/health").send().await {
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
                    // 关闭主窗口 = 退出应用
                    app_handle.exit(0);
                }
            });
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