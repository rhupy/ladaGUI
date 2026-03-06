use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{DragDropEvent, Emitter, WebviewEvent};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

static CANCEL_FLAG: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Serialize, Deserialize)]
pub struct LadaSettings {
    detection_model: String, // "v4-fast" or "v4-accurate"
    max_clip_length: u32,
    crf: u32,
    preset: String,
    prefix: String,
    same_directory: bool,
    output_directory: String,
}

impl Default for LadaSettings {
    fn default() -> Self {
        Self {
            detection_model: "v4-accurate".to_string(),
            max_clip_length: 180,
            crf: 18,
            preset: "medium".to_string(),
            prefix: "[nm]".to_string(),
            same_directory: true,
            output_directory: String::new(),
        }
    }
}

#[derive(Clone, Serialize)]
struct ProgressPayload {
    file_index: usize,
    file_name: String,
    progress: f64,
    status: String, // "processing", "done", "error", "cancelled"
    message: String,
}

#[tauri::command]
async fn check_docker() -> Result<String, String> {
    let output = Command::new("docker")
        .args(["info"])
        .output()
        .await
        .map_err(|e| format!("Docker not found: {}", e))?;

    if output.status.success() {
        // Check GPU support
        let gpu_output = Command::new("docker")
            .args(["run", "--rm", "--gpus", "all", "nvidia/cuda:12.0.0-base-ubuntu22.04", "nvidia-smi", "--query-gpu=name", "--format=csv,noheader"])
            .output()
            .await;

        match gpu_output {
            Ok(o) if o.status.success() => {
                let gpu_name = String::from_utf8_lossy(&o.stdout).trim().to_string();
                Ok(format!("Docker OK, GPU: {}", gpu_name))
            }
            _ => Ok("Docker OK, GPU: not detected".to_string()),
        }
    } else {
        Err("Docker is not running".to_string())
    }
}

#[tauri::command]
async fn update_lada() -> Result<String, String> {
    let output = Command::new("docker")
        .args(["pull", "ladaapp/lada:latest"])
        .output()
        .await
        .map_err(|e| format!("Failed to pull: {}", e))?;

    if output.status.success() {
        let msg = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(msg)
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

#[tauri::command]
async fn cancel_processing() -> Result<(), String> {
    CANCEL_FLAG.store(true, Ordering::SeqCst);
    // Stop any running lada containers
    let _ = Command::new("docker")
        .args(["ps", "-q", "--filter", "ancestor=ladaapp/lada:latest"])
        .output()
        .await
        .map(|output| {
            let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !container_id.is_empty() {
                let _ = std::process::Command::new("docker")
                    .args(["stop", &container_id])
                    .spawn();
            }
        });
    Ok(())
}

#[tauri::command]
async fn process_files(
    app: tauri::AppHandle,
    files: Vec<String>,
    settings: LadaSettings,
) -> Result<(), String> {
    CANCEL_FLAG.store(false, Ordering::SeqCst);

    let progress_re = Regex::new(r"Processing video:\s+(\d+)%").unwrap();
    let tmp_dir = std::env::temp_dir().join("lada-gui-tmp");
    let _ = std::fs::create_dir_all(&tmp_dir);

    for (index, file_path) in files.iter().enumerate() {
        if CANCEL_FLAG.load(Ordering::SeqCst) {
            let _ = app.emit(
                "progress",
                ProgressPayload {
                    file_index: index,
                    file_name: file_name_from_path(file_path),
                    progress: 0.0,
                    status: "cancelled".to_string(),
                    message: "Cancelled by user".to_string(),
                },
            );
            break;
        }

        let input_path = PathBuf::from(file_path);
        let file_name = file_name_from_path(file_path);

        // Determine output path
        let output_dir = if settings.same_directory {
            input_path.parent().unwrap().to_path_buf()
        } else {
            PathBuf::from(&settings.output_directory)
        };

        let stem = input_path.file_stem().unwrap().to_string_lossy();
        let output_filename = format!("{} {}.mp4", settings.prefix, stem);

        // Emit start
        let _ = app.emit(
            "progress",
            ProgressPayload {
                file_index: index,
                file_name: file_name.clone(),
                progress: 0.0,
                status: "processing".to_string(),
                message: "Starting...".to_string(),
            },
        );

        // Prepare docker paths - convert Windows paths to WSL mount paths
        let input_dir_str = to_docker_mount_path(input_path.parent().unwrap().to_str().unwrap());
        let output_dir_str = to_docker_mount_path(output_dir.to_str().unwrap());
        let tmp_dir_str = tmp_dir.to_str().unwrap().to_string();

        // Ensure output dir permissions
        let _ = Command::new("chmod").args(["777", &output_dir_str]).output().await;
        let _ = Command::new("chmod").args(["777", &tmp_dir_str]).output().await;

        let encoder_options = format!("-crf {} -preset {} -x265-params log_level=error", settings.crf, settings.preset);

        let mut cmd = Command::new("docker");
        cmd.args([
            "run", "--rm", "--gpus", "all",
            "-v", &format!("{}:/input", input_dir_str),
            "-v", &format!("{}:/output", output_dir_str),
            "-v", &format!("{}:/tmp", tmp_dir_str),
            "ladaapp/lada:latest",
            "--input", &format!("/input/{}", input_path.file_name().unwrap().to_string_lossy()),
            "--output", &format!("/output/{}", output_filename),
            "--temporary-directory", "/tmp",
            "--mosaic-detection-model", &settings.detection_model,
            "--max-clip-length", &settings.max_clip_length.to_string(),
            "--encoder", "libx265",
            "--encoder-options", &encoder_options,
        ]);

        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| format!("Failed to start Docker: {}", e))?;

        let stderr = child.stderr.take().unwrap();
        let mut reader = BufReader::new(stderr).lines();

        let app_clone = app.clone();
        let file_name_clone = file_name.clone();
        let progress_re_clone = progress_re.clone();

        // Read stderr for progress
        while let Ok(Some(line)) = reader.next_line().await {
            if CANCEL_FLAG.load(Ordering::SeqCst) {
                let _ = child.kill().await;
                break;
            }

            if let Some(caps) = progress_re_clone.captures(&line) {
                if let Ok(pct) = caps[1].parse::<f64>() {
                    let _ = app_clone.emit(
                        "progress",
                        ProgressPayload {
                            file_index: index,
                            file_name: file_name_clone.clone(),
                            progress: pct,
                            status: "processing".to_string(),
                            message: extract_progress_detail(&line),
                        },
                    );
                }
            }
        }

        let status = child.wait().await.map_err(|e| format!("Process error: {}", e))?;

        if CANCEL_FLAG.load(Ordering::SeqCst) {
            break;
        }

        let final_status = if status.success() { "done" } else { "error" };
        let final_msg = if status.success() {
            format!("Saved: {}", output_filename)
        } else {
            format!("Failed with exit code: {:?}", status.code())
        };

        let _ = app.emit(
            "progress",
            ProgressPayload {
                file_index: index,
                file_name: file_name.clone(),
                progress: if status.success() { 100.0 } else { 0.0 },
                status: final_status.to_string(),
                message: final_msg,
            },
        );
    }

    Ok(())
}

fn file_name_from_path(path: &str) -> String {
    PathBuf::from(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

fn to_docker_mount_path(path: &str) -> String {
    // Convert Windows-style paths (X:\...) to WSL mount paths (/mnt/x/...)
    if path.len() >= 2 && path.as_bytes()[1] == b':' {
        let drive = (path.as_bytes()[0] as char).to_lowercase().to_string();
        let rest = path[2..].replace('\\', "/");
        format!("/mnt/{}{}", drive, rest)
    } else {
        path.to_string()
    }
}

fn extract_progress_detail(line: &str) -> String {
    // Extract useful info like "Processed: 01:23 (2500f) | Remaining: 00:45"
    if let Some(pos) = line.find("Processed:") {
        line[pos..].to_string()
    } else {
        line.to_string()
    }
}

#[derive(Clone, Serialize)]
struct DroppedFiles {
    paths: Vec<String>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            check_docker,
            update_lada,
            process_files,
            cancel_processing,
        ])
        .on_webview_event(|webview, event| {
            if let WebviewEvent::DragDrop(DragDropEvent::Drop { paths, .. }) = event {
                let file_paths: Vec<String> = paths
                    .iter()
                    .filter_map(|p| p.to_str().map(|s| s.to_string()))
                    .collect();
                let _ = webview.emit("files-dropped", DroppedFiles { paths: file_paths });
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
