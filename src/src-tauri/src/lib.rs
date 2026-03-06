use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{DragDropEvent, Emitter, WebviewEvent};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

static CANCEL_FLAG: AtomicBool = AtomicBool::new(false);
static LAST_STDERR: Mutex<Option<String>> = Mutex::new(None);

#[derive(Clone, Serialize, Deserialize)]
pub struct LadaSettings {
    detection_model: String,
    max_clip_length: u32,
    crf: u32,
    preset: String,
    prefix: String,
    same_directory: bool,
    output_directory: String,
}

#[derive(Clone, Serialize)]
struct ProgressPayload {
    file_index: usize,
    file_name: String,
    progress: f64,
    status: String,
    message: String,
}

#[derive(Clone, Serialize)]
struct DroppedFiles {
    paths: Vec<String>,
}

#[tauri::command]
async fn check_docker() -> Result<String, String> {
    let output = Command::new("docker")
        .args(["info"])
        .output()
        .await
        .map_err(|e| format!("Docker not found: {}", e))?;

    if output.status.success() {
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
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
}

#[tauri::command]
async fn cancel_processing() -> Result<(), String> {
    CANCEL_FLAG.store(true, Ordering::SeqCst);
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

#[tauri::command]
async fn process_files(
    app: tauri::AppHandle,
    files: Vec<String>,
    settings: LadaSettings,
) -> Result<(), String> {
    CANCEL_FLAG.store(false, Ordering::SeqCst);

    let progress_re = Regex::new(r"Processing video:\s+(\d+)%").unwrap();

    // Use system temp dir for temporary files
    let tmp_dir = std::env::temp_dir().join("lada-gui-tmp");
    let _ = std::fs::create_dir_all(&tmp_dir);

    for (index, file_path) in files.iter().enumerate() {
        if CANCEL_FLAG.load(Ordering::SeqCst) {
            let _ = app.emit("progress", ProgressPayload {
                file_index: index,
                file_name: file_name_from_path(file_path),
                progress: 0.0,
                status: "cancelled".to_string(),
                message: "Cancelled by user".to_string(),
            });
            break;
        }

        let input_path = PathBuf::from(file_path);
        let file_name = file_name_from_path(file_path);

        let output_dir = if settings.same_directory {
            input_path.parent().unwrap().to_path_buf()
        } else {
            PathBuf::from(&settings.output_directory)
        };

        let stem = input_path.file_stem().unwrap().to_string_lossy();
        let output_filename = format!("{} {}.mp4", settings.prefix, stem);

        let _ = app.emit("progress", ProgressPayload {
            file_index: index,
            file_name: file_name.clone(),
            progress: 0.0,
            status: "processing".to_string(),
            message: "Starting...".to_string(),
        });

        // Convert paths for Docker volume mounts
        let input_dir_docker = to_docker_volume_path(
            input_path.parent().unwrap().to_str().unwrap()
        );
        let output_dir_docker = to_docker_volume_path(
            output_dir.to_str().unwrap()
        );
        let tmp_dir_docker = to_docker_volume_path(
            tmp_dir.to_str().unwrap()
        );

        // On Linux/WSL, fix permissions
        if !cfg!(windows) {
            let _ = Command::new("chmod").args(["777", &output_dir_docker]).output().await;
            let _ = Command::new("chmod").args(["777", &tmp_dir_docker]).output().await;
        }

        let encoder_options = format!(
            "-crf {} -preset {} -x265-params log_level=error",
            settings.crf, settings.preset
        );

        let input_file_name = input_path.file_name().unwrap().to_string_lossy().to_string();

        let mut cmd = Command::new("docker");
        cmd.args([
            "run", "--rm", "--gpus", "all",
            "-v", &format!("{}:/input", input_dir_docker),
            "-v", &format!("{}:/output", output_dir_docker),
            "-v", &format!("{}:/tmp", tmp_dir_docker),
            "ladaapp/lada:latest",
            "--input", &format!("/input/{}", input_file_name),
            "--output", &format!("/output/{}", output_filename),
            "--temporary-directory", "/tmp",
            "--mosaic-detection-model", &settings.detection_model,
            "--max-clip-length", &settings.max_clip_length.to_string(),
            "--encoder", "libx265",
            "--encoder-options", &encoder_options,
        ]);

        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        // On Windows, hide the console window for docker process
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
        }

        let mut child = cmd.spawn().map_err(|e| format!("Failed to start Docker: {}", e))?;

        let stderr = child.stderr.take().unwrap();
        let mut reader = BufReader::new(stderr).lines();

        let app_clone = app.clone();
        let file_name_clone = file_name.clone();
        let progress_re_clone = progress_re.clone();

        // Collect stderr for error reporting
        let mut last_stderr_lines: Vec<String> = Vec::new();

        while let Ok(Some(line)) = reader.next_line().await {
            if CANCEL_FLAG.load(Ordering::SeqCst) {
                let _ = child.kill().await;
                break;
            }

            // Keep last 10 lines for error reporting
            last_stderr_lines.push(line.clone());
            if last_stderr_lines.len() > 10 {
                last_stderr_lines.remove(0);
            }

            if let Some(caps) = progress_re_clone.captures(&line) {
                if let Ok(pct) = caps[1].parse::<f64>() {
                    let _ = app_clone.emit("progress", ProgressPayload {
                        file_index: index,
                        file_name: file_name_clone.clone(),
                        progress: pct,
                        status: "processing".to_string(),
                        message: extract_progress_detail(&line),
                    });
                }
            }
        }

        let status = child.wait().await.map_err(|e| format!("Process error: {}", e))?;

        if CANCEL_FLAG.load(Ordering::SeqCst) {
            break;
        }

        let (final_status, final_msg) = if status.success() {
            ("done".to_string(), format!("Saved: {}", output_filename))
        } else {
            let stderr_tail = last_stderr_lines.join("\n");
            ("error".to_string(), format!("Exit code: {:?}\n{}", status.code(), stderr_tail))
        };

        let _ = app.emit("progress", ProgressPayload {
            file_index: index,
            file_name: file_name.clone(),
            progress: if status.success() { 100.0 } else { 0.0 },
            status: final_status,
            message: final_msg,
        });
    }

    Ok(())
}

fn file_name_from_path(path: &str) -> String {
    PathBuf::from(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

fn extract_progress_detail(line: &str) -> String {
    if let Some(pos) = line.find("Processed:") {
        line[pos..].to_string()
    } else {
        line.to_string()
    }
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
                if !file_paths.is_empty() {
                    let _ = webview.emit("files-dropped", DroppedFiles { paths: file_paths });
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
