use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::io::Write;
use tauri::{DragDropEvent, Emitter, Manager, WebviewEvent};
use tokio::io::BufReader;
use tokio::process::Command;
use tokio::sync::Semaphore;

/// Apply CREATE_NO_WINDOW flag on Windows to prevent console window flashing
#[cfg(windows)]
fn hide_window(cmd: &mut Command) -> &mut Command {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(0x08000000)
}

#[cfg(not(windows))]
fn hide_window(cmd: &mut Command) -> &mut Command {
    cmd
}

#[cfg(windows)]
fn hide_window_std(cmd: &mut std::process::Command) -> &mut std::process::Command {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(0x08000000)
}

#[cfg(not(windows))]
fn hide_window_std(cmd: &mut std::process::Command) -> &mut std::process::Command {
    cmd
}

use sysinfo::System;
use std::sync::Mutex;

static CANCEL_FLAG: AtomicBool = AtomicBool::new(false);

static SYS_INFO: std::sync::LazyLock<Mutex<System>> = std::sync::LazyLock::new(|| {
    let mut sys = System::new_all();
    sys.refresh_cpu_all();
    sys.refresh_memory();
    Mutex::new(sys)
});

#[derive(Clone, Serialize)]
struct SystemStats {
    cpu_usage: f32,
    ram_used: u64,
    ram_total: u64,
    ram_percent: f32,
    gpu_usage: u32,
    vram_used: u64,
    vram_total: u64,
    gpu_temp: u32,
    gpu_power: f32,
}

#[tauri::command]
async fn get_system_stats() -> Result<SystemStats, String> {
    // CPU & RAM via sysinfo
    let (cpu_usage, ram_used, ram_total) = {
        let mut sys = SYS_INFO.lock().unwrap();
        sys.refresh_cpu_all();
        sys.refresh_memory();
        let cpu = sys.global_cpu_usage();
        let used = sys.used_memory();
        let total = sys.total_memory();
        (cpu, used, total)
    };

    let ram_percent = if ram_total > 0 {
        (ram_used as f64 / ram_total as f64 * 100.0) as f32
    } else {
        0.0
    };

    // GPU via nvidia-smi
    let (gpu_usage, vram_used, vram_total, gpu_temp, gpu_power) = {
        let mut cmd = tokio::process::Command::new("nvidia-smi");
        cmd.args(["--query-gpu=utilization.gpu,memory.used,memory.total,temperature.gpu,power.draw", "--format=csv,noheader,nounits"]);
        hide_window(&mut cmd);
        match cmd.output().await
        {
            Ok(output) if output.status.success() => {
                let s = String::from_utf8_lossy(&output.stdout);
                let parts: Vec<&str> = s.trim().split(',').map(|p| p.trim()).collect();
                if parts.len() >= 5 {
                    (
                        parts[0].parse::<u32>().unwrap_or(0),
                        parts[1].parse::<u64>().unwrap_or(0),
                        parts[2].parse::<u64>().unwrap_or(0),
                        parts[3].parse::<u32>().unwrap_or(0),
                        parts[4].parse::<f32>().unwrap_or(0.0),
                    )
                } else {
                    (0, 0, 0, 0, 0.0)
                }
            }
            _ => (0, 0, 0, 0, 0.0),
        }
    };

    Ok(SystemStats {
        cpu_usage,
        ram_used: ram_used / (1024 * 1024), // bytes → MB
        ram_total: ram_total / (1024 * 1024),
        ram_percent,
        gpu_usage,
        vram_used,
        vram_total,
        gpu_temp,
        gpu_power,
    })
}

fn write_log(msg: &str) {
    let log_path = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("lada-gui.log")))
        .unwrap_or_else(|| PathBuf::from("lada-gui.log"));
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&log_path) {
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
        let _ = writeln!(f, "[{}] {}", now, msg);
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct LadaSettings {
    detection_model: String,
    #[serde(default = "default_restoration_model")]
    restoration_model: String,
    #[serde(default = "default_fp16")]
    fp16: String, // "auto" | "on" | "off"
    max_clip_length: u32,
    encoder: String,
    crf: u32,
    preset: String,
    prefix: String,
    same_directory: bool,
    output_directory: String,
    delete_original: bool,
    shutdown_after: bool,
    parallel_jobs: u32,
    #[serde(default = "default_memory_limit")]
    memory_limit: u32, // GB per container, 0 = unlimited
}

fn default_memory_limit() -> u32 { 10 }
fn default_restoration_model() -> String { "basicvsrpp-v1.2".to_string() }
fn default_fp16() -> String { "auto".to_string() }

#[derive(Clone, Serialize)]
struct ProgressPayload {
    file_index: usize,
    total_files: usize,
    file_name: String,
    progress: f64,
    status: String,
    message: String,
    remaining: String,
    speed: String,
}

#[derive(Clone, Serialize)]
struct DroppedFiles {
    paths: Vec<String>,
}

#[tauri::command]
async fn check_docker() -> Result<String, String> {
    let mut cmd = Command::new("docker");
    cmd.args(["info"]);
    hide_window(&mut cmd);
    let output = cmd.output().await
        .map_err(|e| format!("Docker not found: {}", e))?;

    if output.status.success() {
        let info_str = String::from_utf8_lossy(&output.stdout);
        let has_nvidia = info_str.contains("nvidia") || info_str.contains("NVIDIA");
        if has_nvidia {
            Ok("Docker OK, GPU: NVIDIA".to_string())
        } else {
            Ok("Docker OK, GPU: not detected".to_string())
        }
    } else {
        Err("Docker is not running".to_string())
    }
}

#[tauri::command]
async fn update_lada() -> Result<String, String> {
    let mut cmd = Command::new("docker");
    cmd.args(["pull", "ladaapp/lada:latest"]);
    hide_window(&mut cmd);
    let output = cmd.output().await
        .map_err(|e| format!("Failed to pull: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

#[tauri::command]
async fn cancel_processing() -> Result<(), String> {
    CANCEL_FLAG.store(true, Ordering::SeqCst);
    // Stop ALL running lada containers (fixes #14: multiple parallel jobs)
    let mut cmd = Command::new("docker");
    cmd.args(["ps", "-q", "--filter", "ancestor=ladaapp/lada:latest"]);
    hide_window(&mut cmd);
    if let Ok(output) = cmd.output().await {
        let ids = String::from_utf8_lossy(&output.stdout);
        for id in ids.lines() {
            let id = id.trim();
            if !id.is_empty() {
                let mut stop_cmd = std::process::Command::new("docker");
                stop_cmd.args(["stop", id]);
                hide_window_std(&mut stop_cmd);
                let _ = stop_cmd.spawn();
            }
        }
    }
    Ok(())
}

#[tauri::command]
async fn pause_processing() -> Result<(), String> {
    let mut cmd = Command::new("docker");
    cmd.args(["ps", "-q", "--filter", "ancestor=ladaapp/lada:latest"]);
    hide_window(&mut cmd);
    if let Ok(output) = cmd.output().await {
        let ids = String::from_utf8_lossy(&output.stdout);
        for id in ids.lines() {
            let id = id.trim();
            if !id.is_empty() {
                let mut pause_cmd = Command::new("docker");
                pause_cmd.args(["pause", id]);
                hide_window(&mut pause_cmd);
                let _ = pause_cmd.output().await;
            }
        }
    }
    Ok(())
}

#[tauri::command]
async fn resume_processing() -> Result<(), String> {
    let mut cmd = Command::new("docker");
    cmd.args(["ps", "-q", "--filter", "ancestor=ladaapp/lada:latest", "--filter", "status=paused"]);
    hide_window(&mut cmd);
    if let Ok(output) = cmd.output().await {
        let ids = String::from_utf8_lossy(&output.stdout);
        for id in ids.lines() {
            let id = id.trim();
            if !id.is_empty() {
                let mut unpause_cmd = Command::new("docker");
                unpause_cmd.args(["unpause", id]);
                hide_window(&mut unpause_cmd);
                let _ = unpause_cmd.output().await;
            }
        }
    }
    Ok(())
}

#[tauri::command]
async fn save_settings(app: tauri::AppHandle, settings: LadaSettings) -> Result<(), String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("settings.json");
    let json = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}

#[tauri::command]
async fn load_settings(app: tauri::AppHandle) -> Result<Option<LadaSettings>, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    let path = dir.join("settings.json");
    if path.exists() {
        let json = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let settings: LadaSettings = serde_json::from_str(&json).map_err(|e| e.to_string())?;
        Ok(Some(settings))
    } else {
        Ok(None)
    }
}

/// Convert a native OS path to Docker volume mount format.
/// On Windows: `X:\foo\bar` → `/x/foo/bar` (Docker Desktop format)
/// On Linux/WSL: pass through as-is
fn to_docker_volume_path(path: &str) -> String {
    if cfg!(windows) {
        // Docker Desktop on Windows expects //x/path or /x/path
        if path.len() >= 2 && path.as_bytes()[1] == b':' {
            let drive = (path.as_bytes()[0] as char).to_lowercase().to_string();
            let rest = path[2..].replace('\\', "/");
            format!("/{}{}", drive, rest)
        } else {
            path.replace('\\', "/")
        }
    } else {
        // Running under WSL - convert Windows paths to /mnt/x/...
        if path.len() >= 2 && path.as_bytes()[1] == b':' {
            let drive = (path.as_bytes()[0] as char).to_lowercase().to_string();
            let rest = path[2..].replace('\\', "/");
            format!("/mnt/{}{}", drive, rest)
        } else {
            path.to_string()
        }
    }
}

async fn process_single_file(
    app: tauri::AppHandle,
    file_path: String,
    index: usize,
    total_files: usize,
    settings: LadaSettings,
) {
    let progress_re = Regex::new(r"Processing video:\s+(\d+)%").unwrap();
    let remaining_re = Regex::new(r"Remaining:\s*(\S+)").unwrap();
    let speed_re = Regex::new(r"Speed:\s*(\S+)").unwrap();

    let input_path = PathBuf::from(&file_path);
    let file_name = file_name_from_path(&file_path);

    let output_dir = if settings.same_directory {
        input_path.parent().unwrap().to_path_buf()
    } else {
        PathBuf::from(&settings.output_directory)
    };

    let stem = input_path.file_stem().unwrap().to_string_lossy();
    let output_filename = format!("{} {}.mp4", settings.prefix, stem);

    // Use per-job temp dir to avoid conflicts between parallel jobs
    let tmp_dir = std::env::temp_dir().join(format!("lada-gui-tmp-{}", index));
    let _ = std::fs::create_dir_all(&tmp_dir);

    let _ = app.emit("progress", ProgressPayload {
        file_index: index, total_files,
        file_name: file_name.clone(), progress: 0.0,
        status: "processing".to_string(), message: "Starting...".to_string(),
        remaining: String::new(), speed: String::new(),
    });

    let input_dir_docker = to_docker_volume_path(input_path.parent().unwrap().to_str().unwrap());
    let output_dir_docker = to_docker_volume_path(output_dir.to_str().unwrap());
    let tmp_dir_docker = to_docker_volume_path(tmp_dir.to_str().unwrap());

    if !cfg!(windows) {
        let _ = Command::new("chmod").args(["777", &output_dir_docker]).output().await;
        let _ = Command::new("chmod").args(["777", &tmp_dir_docker]).output().await;
    }

    let encoder = &settings.encoder;
    let encoder_options = if encoder == "hevc_nvenc" || encoder == "h264_nvenc" {
        format!("-preset {} -cq {}", settings.preset, settings.crf)
    } else {
        format!("-crf {} -preset {} -x265-params log_level=error", settings.crf, settings.preset)
    };

    let input_file_name = input_path.file_name().unwrap().to_string_lossy().to_string();

    let mut args: Vec<String> = vec![
        "run".into(), "--rm".into(), "--gpus".into(), "all".into(),
    ];
    // Memory limit per container (prevents heap corruption / segfault)
    if settings.memory_limit > 0 {
        args.push("--memory".into());
        args.push(format!("{}g", settings.memory_limit));
    }
    args.extend([
        "-v".into(), format!("{}:/input", input_dir_docker),
        "-v".into(), format!("{}:/output", output_dir_docker),
        "-v".into(), format!("{}:/tmp", tmp_dir_docker),
        "-e".into(), "NVIDIA_DRIVER_CAPABILITIES=compute,video,utility".into(),
        "ladaapp/lada:latest".into(),
        "--input".into(), format!("/input/{}", input_file_name),
        "--output".into(), format!("/output/{}", output_filename),
        "--temporary-directory".into(), "/tmp".into(),
        "--mosaic-detection-model".into(), settings.detection_model.clone(),
        "--mosaic-restoration-model".into(), settings.restoration_model.clone(),
        "--max-clip-length".into(), settings.max_clip_length.to_string(),
        "--encoder".into(), encoder.clone(),
        "--encoder-options".into(), encoder_options.clone(),
    ]);
    // FP16: "auto" lets Lada decide based on GPU capabilities; otherwise force on/off
    match settings.fp16.as_str() {
        "on" => args.push("--fp16".into()),
        "off" => args.push("--no-fp16".into()),
        _ => {}
    }

    // Retry loop: on failure, log error and retry indefinitely until cancel
    let mut attempt = 0u32;
    loop {
        attempt += 1;

        if CANCEL_FLAG.load(Ordering::SeqCst) { break; }

        let mut cmd = Command::new("docker");
        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        cmd.args(&arg_refs);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        hide_window(&mut cmd);

        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let _ = app.emit("progress", ProgressPayload {
                    file_index: index, total_files,
                    file_name: file_name.clone(), progress: 0.0,
                    status: "processing".to_string(),
                    message: format!("Attempt {} failed to start: {}. Retrying in 30s...", attempt, e),
                    remaining: String::new(), speed: String::new(),
                });
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                continue;
            }
        };

        let stderr = child.stderr.take().unwrap();
        let mut reader = BufReader::new(stderr);
        let mut last_stderr_lines: Vec<String> = Vec::new();

        let app_clone = app.clone();
        let file_name_clone = file_name.clone();
        let progress_re_clone = progress_re.clone();

        let mut line_buf = Vec::new();
        loop {
            let mut byte = [0u8; 1];
            match tokio::io::AsyncReadExt::read(&mut reader, &mut byte).await {
                Ok(0) => break,
                Ok(_) => {
                    if byte[0] == b'\r' || byte[0] == b'\n' {
                        if !line_buf.is_empty() {
                            let line = String::from_utf8_lossy(&line_buf).to_string();
                            line_buf.clear();

                            if CANCEL_FLAG.load(Ordering::SeqCst) {
                                let _ = child.kill().await;
                                break;
                            }

                            last_stderr_lines.push(line.clone());
                            if last_stderr_lines.len() > 10 { last_stderr_lines.remove(0); }

                            if let Some(caps) = progress_re_clone.captures(&line) {
                                if let Ok(pct) = caps[1].parse::<f64>() {
                                    let rem = remaining_re.captures(&line)
                                        .map(|c| c[1].to_string()).unwrap_or_default();
                                    let spd = speed_re.captures(&line)
                                        .map(|c| c[1].to_string()).unwrap_or_default();
                                    let msg = if attempt > 1 {
                                        format!("[Retry #{}] {}", attempt, extract_progress_detail(&line))
                                    } else {
                                        extract_progress_detail(&line)
                                    };
                                    let _ = app_clone.emit("progress", ProgressPayload {
                                        file_index: index, total_files,
                                        file_name: file_name_clone.clone(),
                                        progress: pct, status: "processing".to_string(),
                                        message: msg, remaining: rem, speed: spd,
                                    });
                                }
                            }
                        }
                    } else {
                        line_buf.push(byte[0]);
                    }
                }
                Err(_) => break,
            }
        }

        let status = match child.wait().await {
            Ok(s) => s,
            Err(e) => {
                write_log(&format!("RETRY file=\"{}\" attempt={} error: process wait failed: {}", file_name, attempt, e));
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                continue;
            }
        };

        if CANCEL_FLAG.load(Ordering::SeqCst) { break; }

        let output_file_path = output_dir.join(&output_filename);
        let output_valid = output_file_path.exists()
            && std::fs::metadata(&output_file_path).map(|m| m.len() > 1000).unwrap_or(false);

        if status.success() && output_valid {
            let final_msg = if settings.delete_original {
                match std::fs::remove_file(&input_path) {
                    Ok(_) => format!("Saved: {} (original deleted)", output_filename),
                    Err(e) => format!("Saved: {} (failed to delete original: {})", output_filename, e),
                }
            } else {
                format!("Saved: {}", output_filename)
            };
            let _ = app.emit("progress", ProgressPayload {
                file_index: index, total_files,
                file_name: file_name.clone(), progress: 100.0,
                status: "done".to_string(), message: final_msg,
                remaining: String::new(), speed: String::new(),
            });
            write_log(&format!("DONE file=\"{}\" attempts={} output=\"{}\"", file_name, attempt, output_filename));
            break;
        } else {
            let stderr_tail = last_stderr_lines.join("\n");
            let error_msg = if status.success() && !output_valid {
                format!("Attempt {}: output missing. Retrying in 30s...\n{}", attempt, stderr_tail)
            } else {
                format!("Attempt {}: exit code {:?}. Retrying in 30s...\n{}", attempt, status.code(), stderr_tail)
            };
            write_log(&format!("RETRY file=\"{}\" attempt={} error: {}", file_name, attempt, error_msg.replace('\n', " | ")));
            let _ = app.emit("progress", ProgressPayload {
                file_index: index, total_files,
                file_name: file_name.clone(), progress: 0.0,
                status: "processing".to_string(), message: error_msg,
                remaining: String::new(), speed: String::new(),
            });
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            let _ = std::fs::remove_file(output_dir.join(&output_filename));
        }
    }
}

#[tauri::command]
async fn process_files(
    app: tauri::AppHandle,
    files: Vec<String>,
    settings: LadaSettings,
) -> Result<(), String> {
    CANCEL_FLAG.store(false, Ordering::SeqCst);

    let total_files = files.len();
    let parallel = (settings.parallel_jobs.max(1).min(99)) as usize;
    let semaphore = Arc::new(Semaphore::new(parallel));

    let mut handles = Vec::new();

    for (index, file_path) in files.into_iter().enumerate() {
        if CANCEL_FLAG.load(Ordering::SeqCst) {
            let _ = app.emit("progress", ProgressPayload {
                file_index: index, total_files,
                file_name: file_name_from_path(&file_path),
                progress: 0.0, status: "cancelled".to_string(),
                message: "Cancelled by user".to_string(),
                remaining: String::new(), speed: String::new(),
            });
            break;
        }

        let permit = semaphore.clone().acquire_owned().await.unwrap();
        let app_clone = app.clone();
        let settings_clone = settings.clone();

        let handle = tokio::spawn(async move {
            process_single_file(app_clone, file_path, index, total_files, settings_clone).await;
            drop(permit);
        });
        handles.push(handle);
    }

    // Wait for all tasks to complete
    for handle in handles {
        let _ = handle.await;
    }

    Ok(())
}

#[tauri::command]
async fn shutdown_pc() -> Result<(), String> {
    #[cfg(windows)]
    {
        let mut sd = std::process::Command::new("shutdown");
        sd.args(["/s", "/f", "/t", "10"]);
        hide_window_std(&mut sd);
        let _ = sd.spawn();
    }
    #[cfg(not(windows))]
    { let _ = std::process::Command::new("shutdown").args(["-h", "now"]).spawn(); }
    Ok(())
}

fn file_name_from_path(path: &str) -> String {
    PathBuf::from(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

fn extract_progress_detail(line: &str) -> String {
    // Take everything from "Processed:" onward...
    let detail = match line.find("Processed:") {
        Some(pos) => &line[pos..],
        None => line,
    };
    // ...but stop before "Remaining"/"Speed" — those are emitted in dedicated fields
    // so the left-hand label shows only the processed count.
    let cut = detail
        .find("Remaining")
        .or_else(|| detail.find("Speed"))
        .unwrap_or(detail.len());
    detail[..cut]
        .trim()
        .trim_end_matches(|c: char| c == ',' || c == '|' || c == '[' || c.is_whitespace())
        .to_string()
}

fn filter_video_paths(args: &[String]) -> Vec<String> {
    let video_exts = ["mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "ts"];
    args.iter()
        .filter(|a| {
            let lower = a.to_lowercase();
            video_exts.iter().any(|ext| lower.ends_with(&format!(".{}", ext)))
        })
        .cloned()
        .collect()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            // Second instance launched with file args → send to existing instance
            let paths = filter_video_paths(&argv);
            if !paths.is_empty() {
                let _ = app.emit("files-dropped", DroppedFiles { paths });
            }
            // Focus the existing window
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            check_docker,
            update_lada,
            process_files,
            cancel_processing,
            pause_processing,
            resume_processing,
            save_settings,
            load_settings,
            get_system_stats,
            shutdown_pc,
        ])
        .setup(|app| {
            // Handle CLI args on first launch
            let args: Vec<String> = std::env::args().collect();
            let paths = filter_video_paths(&args);
            if !paths.is_empty() {
                let app_handle = app.handle().clone();
                // Delay to ensure frontend is ready
                tokio::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    let _ = app_handle.emit("files-dropped", DroppedFiles { paths });
                });
            }
            Ok(())
        })
        .on_webview_event(|webview, event| {
            if let WebviewEvent::DragDrop(DragDropEvent::Drop { paths, .. }) = event {
                let file_paths: Vec<String> = paths
                    .iter()
                    .filter_map(|p| p.to_str().map(|s| s.to_string()))
                    .collect();
                if !file_paths.is_empty() {
                    let _ = webview.emit("files-dropped", DroppedFiles { paths: file_paths });
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
