use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{DragDropEvent, Emitter, WebviewEvent};
use tokio::io::BufReader;
use tokio::process::Command;

static CANCEL_FLAG: AtomicBool = AtomicBool::new(false);

#[derive(Clone, Serialize, Deserialize)]
pub struct LadaSettings {
    detection_model: String,
    max_clip_length: u32,
    encoder: String,
    crf: u32,
    preset: String,
    prefix: String,
    same_directory: bool,
    output_directory: String,
    delete_original: bool,
}

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
    let output = Command::new("docker")
        .args(["info"])
        .output()
        .await
        .map_err(|e| format!("Docker not found: {}", e))?;

    if output.status.success() {
        let gpu_output = Command::new("docker")
            .args(["run", "--rm", "--gpus", "all", "ladaapp/lada:latest", "nvidia-smi", "--query-gpu=name", "--format=csv,noheader"])
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
    let remaining_re = Regex::new(r"Remaining:\s*(\S+)").unwrap();
    let speed_re = Regex::new(r"Speed:\s*(\S+)").unwrap();
    let total_files = files.len();

    // Use system temp dir for temporary files
    let tmp_dir = std::env::temp_dir().join("lada-gui-tmp");
    let _ = std::fs::create_dir_all(&tmp_dir);

    for (index, file_path) in files.iter().enumerate() {
        if CANCEL_FLAG.load(Ordering::SeqCst) {
            let _ = app.emit("progress", ProgressPayload {
                file_index: index,
                total_files,
                file_name: file_name_from_path(file_path),
                progress: 0.0,
                status: "cancelled".to_string(),
                message: "Cancelled by user".to_string(),
                remaining: String::new(),
                speed: String::new(),
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
            total_files,
            file_name: file_name.clone(),
            progress: 0.0,
            status: "processing".to_string(),
            message: "Starting...".to_string(),
            remaining: String::new(),
            speed: String::new(),
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

        // Build encoder options based on selected encoder
        let encoder = &settings.encoder;
        let encoder_options = if encoder == "hevc_nvenc" || encoder == "h264_nvenc" {
            format!("-preset {} -cq {}", settings.preset, settings.crf)
        } else {
            format!("-crf {} -preset {} -x265-params log_level=error", settings.crf, settings.preset)
        };

        let input_file_name = input_path.file_name().unwrap().to_string_lossy().to_string();

        let mut cmd = Command::new("docker");
        let mut args: Vec<String> = vec![
            "run".into(), "--rm".into(), "--gpus".into(), "all".into(),
            "-v".into(), format!("{}:/input", input_dir_docker),
            "-v".into(), format!("{}:/output", output_dir_docker),
            "-v".into(), format!("{}:/tmp", tmp_dir_docker),
        ];

        // Mount NVENC libraries for GPU encoding
        // Docker Desktop (Windows) and WSL2 both use a Linux VM where /usr/lib/wsl/lib exists
        args.push("-v".into());
        args.push("/usr/lib/wsl/lib:/usr/lib/wsl/lib".into());
        args.push("-e".into());
        args.push("LD_LIBRARY_PATH=/usr/lib/wsl/lib".into());

        args.extend([
            "ladaapp/lada:latest".into(),
            "--input".into(), format!("/input/{}", input_file_name),
            "--output".into(), format!("/output/{}", output_filename),
            "--temporary-directory".into(), "/tmp".into(),
            "--mosaic-detection-model".into(), settings.detection_model.clone(),
            "--max-clip-length".into(), settings.max_clip_length.to_string(),
            "--encoder".into(), encoder.clone(),
            "--encoder-options".into(), encoder_options.clone(),
        ]);

        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        cmd.args(&arg_refs);
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
        let mut reader = BufReader::new(stderr);

        let app_clone = app.clone();
        let file_name_clone = file_name.clone();
        let progress_re_clone = progress_re.clone();

        // Collect stderr for error reporting
        let mut last_stderr_lines: Vec<String> = Vec::new();

        // Read byte-by-byte to handle \r (carriage return) progress updates
        // Lada outputs "Processing video: XX%...\r" without newlines
        let mut line_buf = Vec::new();
        loop {
            let mut byte = [0u8; 1];
            match tokio::io::AsyncReadExt::read(&mut reader, &mut byte).await {
                Ok(0) => break, // EOF
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
                            if last_stderr_lines.len() > 10 {
                                last_stderr_lines.remove(0);
                            }

                            if let Some(caps) = progress_re_clone.captures(&line) {
                                if let Ok(pct) = caps[1].parse::<f64>() {
                                    let rem = remaining_re.captures(&line)
                                        .map(|c| c[1].to_string())
                                        .unwrap_or_default();
                                    let spd = speed_re.captures(&line)
                                        .map(|c| c[1].to_string())
                                        .unwrap_or_default();
                                    let _ = app_clone.emit("progress", ProgressPayload {
                                        file_index: index,
                                        total_files,
                                        file_name: file_name_clone.clone(),
                                        progress: pct,
                                        status: "processing".to_string(),
                                        message: extract_progress_detail(&line),
                                        remaining: rem,
                                        speed: spd,
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

        let status = child.wait().await.map_err(|e| format!("Process error: {}", e))?;

        if CANCEL_FLAG.load(Ordering::SeqCst) {
            break;
        }

        // Verify output file actually exists and has content
        let output_file_path = output_dir.join(&output_filename);
        let output_valid = output_file_path.exists()
            && std::fs::metadata(&output_file_path).map(|m| m.len() > 1000).unwrap_or(false);

        let (final_status, final_msg) = if status.success() && output_valid {
            // Delete original file if enabled
            if settings.delete_original {
                match std::fs::remove_file(&input_path) {
                    Ok(_) => ("done".to_string(), format!("Saved: {} (original deleted)", output_filename)),
                    Err(e) => ("done".to_string(), format!("Saved: {} (failed to delete original: {})", output_filename, e)),
                }
            } else {
                ("done".to_string(), format!("Saved: {}", output_filename))
            }
        } else if status.success() && !output_valid {
            let stderr_tail = last_stderr_lines.join("\n");
            ("error".to_string(), format!("Process exited OK but output file missing or empty.\n{}", stderr_tail))
        } else {
            let stderr_tail = last_stderr_lines.join("\n");
            ("error".to_string(), format!("Exit code: {:?}\n{}", status.code(), stderr_tail))
        };

        let _ = app.emit("progress", ProgressPayload {
            file_index: index,
            total_files,
            file_name: file_name.clone(),
            progress: if status.success() { 100.0 } else { 0.0 },
            status: final_status,
            message: final_msg,
            remaining: String::new(),
            speed: String::new(),
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
