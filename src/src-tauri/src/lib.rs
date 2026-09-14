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

/// Max Lada clip window for VR (per-eye 4K) passes. The 2D default (up to 180)
/// needs tens of GB at 4K and OOM-kills the container; 20 keeps RAM/VRAM
/// bounded (~7 GB, verified) while retaining enough temporal context.
const VR_MAX_CLIP_LENGTH: u32 = 20;

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

/// Build the `docker run ... ladaapp/lada` argument vector for one Lada job.
/// Mounts are parameterized so the same builder serves the normal 2D path
/// (input dir / output dir separate) and the VR sub-passes (all pointing at /tmp).
fn build_lada_args(
    input_dir_docker: &str,
    output_dir_docker: &str,
    tmp_dir_docker: &str,
    input_file_name: &str,
    output_filename: &str,
    settings: &LadaSettings,
    memory_limit_gb: u32,
    clip_length: u32,
) -> Vec<String> {
    let encoder = &settings.encoder;
    let encoder_options = if encoder == "hevc_nvenc" || encoder == "h264_nvenc" {
        format!("-preset {} -cq {}", settings.preset, settings.crf)
    } else {
        format!("-crf {} -preset {} -x265-params log_level=error", settings.crf, settings.preset)
    };

    let mut args: Vec<String> = vec![
        "run".into(), "--rm".into(), "--gpus".into(), "all".into(),
    ];
    // Memory limit per container (prevents heap corruption / segfault).
    // VR passes pass 0 (unlimited): each split eye is 4K and OOMs at the
    // normal 2D limit (exit 137 crash-loop), so they run uncapped.
    if memory_limit_gb > 0 {
        args.push("--memory".into());
        args.push(format!("{}g", memory_limit_gb));
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
        "--max-clip-length".into(), clip_length.to_string(),
        "--encoder".into(), encoder.clone(),
        "--encoder-options".into(), encoder_options.clone(),
    ]);
    // FP16: "auto" lets Lada decide based on GPU capabilities; otherwise force on/off
    match settings.fp16.as_str() {
        "on" => args.push("--fp16".into()),
        "off" => args.push("--no-fp16".into()),
        _ => {}
    }
    args
}

/// Probe (width, height, duration_seconds) of the first video stream using
/// ffprobe *inside* the lada docker image (so the host needs no ffmpeg).
async fn docker_probe_dims(input_dir_docker: &str, input_file_name: &str) -> Option<(u32, u32, f64)> {
    let args: Vec<String> = vec![
        "run".into(), "--rm".into(),
        "-v".into(), format!("{}:/input", input_dir_docker),
        "--entrypoint".into(), "ffprobe".into(),
        "ladaapp/lada:latest".into(),
        "-v".into(), "error".into(),
        "-select_streams".into(), "v:0".into(),
        "-show_entries".into(), "stream=width,height".into(),
        "-show_entries".into(), "format=duration".into(),
        "-of".into(), "default=noprint_wrappers=1:nokey=1".into(),
        format!("/input/{}", input_file_name),
    ];
    let mut cmd = Command::new("docker");
    cmd.args(&args);
    hide_window(&mut cmd);
    let out = cmd.output().await.ok()?;
    if !out.status.success() { return None; }
    let s = String::from_utf8_lossy(&out.stdout);
    let nums: Vec<&str> = s.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
    if nums.len() < 2 { return None; }
    let w = nums[0].parse::<u32>().ok()?;
    let h = nums[1].parse::<u32>().ok()?;
    let dur = nums.get(2).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
    Some((w, h, dur))
}

/// Average SSIM between the left and right halves over ~10 frames sampled at
/// `seek_sec`, computed via ffmpeg inside the lada image. A true side-by-side
/// stereo pair scores high (~0.6 on real VR here); unrelated 2D halves score low.
async fn docker_lr_ssim(input_dir_docker: &str, input_file_name: &str, seek_sec: f64) -> Option<f64> {
    let filter = "[0:v]crop=iw/2:ih:0:0,scale=256:256,format=gray[l];\
[0:v]crop=iw/2:ih:iw/2:0,scale=256:256,format=gray[r];[l][r]ssim".to_string();
    let args: Vec<String> = vec![
        "run".into(), "--rm".into(), "--gpus".into(), "all".into(),
        "-v".into(), format!("{}:/input", input_dir_docker),
        "--entrypoint".into(), "ffmpeg".into(),
        "ladaapp/lada:latest".into(),
        "-nostdin".into(),
        "-ss".into(), format!("{:.3}", seek_sec),
        "-i".into(), format!("/input/{}", input_file_name),
        "-an".into(),
        "-frames:v".into(), "10".into(),
        "-filter_complex".into(), filter,
        "-f".into(), "null".into(), "-".into(),
    ];
    let mut cmd = Command::new("docker");
    cmd.args(&args);
    hide_window(&mut cmd);
    let out = cmd.output().await.ok()?;
    let s = String::from_utf8_lossy(&out.stderr);
    // ffmpeg's ssim filter logs "... All:<avg> (..)" to stderr at EOF.
    let re = Regex::new(r"All:([0-9.]+)").ok()?;
    let mut last: Option<f64> = None;
    for cap in re.captures_iter(&s) {
        if let Ok(v) = cap[1].parse::<f64>() { last = Some(v); }
    }
    last
}

/// Auto-detect a left/right split (SBS) VR video. Two-stage gate keeps the
/// common 2D case cheap: a non-2:1 aspect exits immediately after one probe;
/// only ~2:1 candidates pay for the extra SSIM sample.
/// Returns Some(duration_seconds) when the file is VR (SBS), None otherwise.
async fn detect_sbs_vr(input_dir_docker: &str, input_file_name: &str) -> Option<f64> {
    let (w, h, dur) = docker_probe_dims(input_dir_docker, input_file_name).await?;
    if h == 0 { return None; }
    let ratio = w as f64 / h as f64;
    // 180° SBS is ~2:1 (two near-square eyes). Excludes 16:9 (1.78) and 2.39 scope.
    if !(ratio > 1.9 && ratio < 2.1) { return None; }
    let seek = if dur > 20.0 { dur / 2.0 } else { 0.0 };
    match docker_lr_ssim(input_dir_docker, input_file_name, seek).await {
        Some(score) => {
            let is_vr = score >= 0.40;
            write_log(&format!("VR-DETECT file=\"{}\" {}x{} ratio={:.3} ssim={:.3} -> {}",
                input_file_name, w, h, ratio, score, if is_vr { "VR(SBS)" } else { "2D" }));
            if is_vr { Some(dur) } else { None }
        }
        None => None,
    }
}

/// Run an ffmpeg step (split/merge) inside the image, streaming ffmpeg's own
/// `-progress` output (on stdout) into a mapped [pct_base, pct_base+pct_span]
/// range so the UI shows real movement instead of appearing frozen.
async fn run_ffmpeg_progress(
    app: &tauri::AppHandle,
    args: &[String],
    duration_sec: f64,
    index: usize,
    total_files: usize,
    file_name: &str,
    label: &str,
    pct_base: f64,
    pct_span: f64,
) -> Result<(), String> {
    let mut cmd = Command::new("docker");
    cmd.args(args);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    hide_window(&mut cmd);

    let mut child = cmd.spawn().map_err(|e| format!("{}spawn failed: {}", label, e))?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    // Drain stderr concurrently (holds any error text for diagnostics).
    let stderr_task = tokio::spawn(async move {
        let mut buf = Vec::new();
        let _ = tokio::io::AsyncReadExt::read_to_end(&mut BufReader::new(stderr), &mut buf).await;
        String::from_utf8_lossy(&buf).to_string()
    });

    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    loop {
        line.clear();
        let n = tokio::io::AsyncBufReadExt::read_line(&mut reader, &mut line).await.unwrap_or(0);
        if n == 0 { break; }
        if CANCEL_FLAG.load(Ordering::SeqCst) { let _ = child.kill().await; break; }
        let l = line.trim();
        if let Some(v) = l.strip_prefix("out_time_us=") {
            if let Ok(us) = v.parse::<f64>() {
                if duration_sec > 0.0 {
                    let frac = (us / 1_000_000.0 / duration_sec).clamp(0.0, 1.0);
                    let mapped = pct_base + frac * pct_span;
                    let _ = app.emit("progress", ProgressPayload {
                        file_index: index, total_files,
                        file_name: file_name.to_string(), progress: mapped,
                        status: "processing".to_string(),
                        message: format!("{}{:.0}%", label, frac * 100.0),
                        remaining: String::new(), speed: String::new(),
                    });
                }
            }
        }
    }

    let status = child.wait().await.map_err(|e| format!("{}wait failed: {}", label, e))?;
    let err = stderr_task.await.unwrap_or_default();
    if status.success() {
        Ok(())
    } else if CANCEL_FLAG.load(Ordering::SeqCst) {
        Err("cancelled".to_string())
    } else {
        Err(err.lines().rev().take(5).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join(" | "))
    }
}

/// Build the ffmpeg args that split the input into /tmp/vr_left.mp4 and
/// /tmp/vr_right.mp4 in a single decode pass (NVENC, high quality, no audio).
/// `-progress pipe:1 -nostats` streams machine-readable progress on stdout.
fn split_lr_args(input_dir_docker: &str, work_dir_docker: &str, input_file_name: &str) -> Vec<String> {
    vec![
        "run".into(), "--rm".into(), "--gpus".into(), "all".into(),
        "-e".into(), "NVIDIA_DRIVER_CAPABILITIES=compute,video,utility".into(),
        "-v".into(), format!("{}:/input", input_dir_docker),
        "-v".into(), format!("{}:/tmp", work_dir_docker),
        "--entrypoint".into(), "ffmpeg".into(),
        "ladaapp/lada:latest".into(),
        "-nostdin".into(), "-v".into(), "error".into(), "-progress".into(), "pipe:1".into(), "-nostats".into(), "-y".into(),
        "-i".into(), format!("/input/{}", input_file_name),
        "-filter_complex".into(),
        "[0:v]split=2[a][b];[a]crop=iw/2:ih:0:0[l];[b]crop=iw/2:ih:iw/2:0[r]".into(),
        "-map".into(), "[l]".into(), "-c:v".into(), "hevc_nvenc".into(),
        "-preset".into(), "p5".into(), "-cq".into(), "18".into(), "-an".into(), "/tmp/vr_left.mp4".into(),
        "-map".into(), "[r]".into(), "-c:v".into(), "hevc_nvenc".into(),
        "-preset".into(), "p5".into(), "-cq".into(), "18".into(), "-an".into(), "/tmp/vr_right.mp4".into(),
    ]
}

/// Build the ffmpeg args that recombine the two processed halves (hstack) and
/// mux the original audio back in, re-encoding video with the user's encoder.
fn merge_lr_args(
    input_dir_docker: &str,
    work_dir_docker: &str,
    output_dir_docker: &str,
    orig_file_name: &str,
    output_filename: &str,
    settings: &LadaSettings,
) -> Vec<String> {
    let encoder = &settings.encoder;
    let mut venc: Vec<String> = vec!["-c:v".into(), encoder.clone()];
    if encoder == "hevc_nvenc" || encoder == "h264_nvenc" {
        venc.extend(["-preset".into(), settings.preset.clone(), "-cq".into(), settings.crf.to_string()]);
    } else {
        venc.extend(["-crf".into(), settings.crf.to_string(), "-preset".into(), settings.preset.clone(),
            "-x265-params".into(), "log_level=error".into()]);
    }

    let mut args: Vec<String> = vec![
        "run".into(), "--rm".into(), "--gpus".into(), "all".into(),
        "-e".into(), "NVIDIA_DRIVER_CAPABILITIES=compute,video,utility".into(),
        "-v".into(), format!("{}:/input", input_dir_docker),
        "-v".into(), format!("{}:/tmp", work_dir_docker),
        "-v".into(), format!("{}:/output", output_dir_docker),
        "--entrypoint".into(), "ffmpeg".into(),
        "ladaapp/lada:latest".into(),
        "-nostdin".into(), "-v".into(), "error".into(), "-progress".into(), "pipe:1".into(), "-nostats".into(), "-y".into(),
        "-i".into(), "/tmp/vr_left_out.mp4".into(),
        "-i".into(), "/tmp/vr_right_out.mp4".into(),
        "-i".into(), format!("/input/{}", orig_file_name),
        "-filter_complex".into(), "[0:v][1:v]hstack=inputs=2[v]".into(),
        "-map".into(), "[v]".into(), "-map".into(), "2:a?".into(),
    ];
    args.extend(venc);
    args.extend(["-c:a".into(), "copy".into(), format!("/output/{}", output_filename)]);
    args
}

/// Run one Lada job (with infinite retry-until-cancel), streaming progress
/// mapped into [pct_base, pct_base + pct_span] of the overall file progress.
/// `expected_output` is the host path that must exist (>1KB) to count as success.
/// Returns true on success, false if cancelled.
async fn run_lada_pass(
    app: &tauri::AppHandle,
    args: &[String],
    expected_output: &std::path::Path,
    index: usize,
    total_files: usize,
    file_name: &str,
    label: &str,
    pct_base: f64,
    pct_span: f64,
) -> bool {
    let progress_re = Regex::new(r"Processing video:\s+(\d+)%").unwrap();
    let remaining_re = Regex::new(r"Remaining:\s*(\S+)").unwrap();
    let speed_re = Regex::new(r"Speed:\s*(\S+)").unwrap();

    let mut attempt = 0u32;
    loop {
        attempt += 1;
        if CANCEL_FLAG.load(Ordering::SeqCst) { return false; }

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
                    file_name: file_name.to_string(), progress: pct_base,
                    status: "processing".to_string(),
                    message: format!("{}Attempt {} failed to start: {}. Retrying in 30s...", label, attempt, e),
                    remaining: String::new(), speed: String::new(),
                });
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                continue;
            }
        };

        let stderr = child.stderr.take().unwrap();
        let mut reader = BufReader::new(stderr);
        let mut last_stderr_lines: Vec<String> = Vec::new();

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

                            if let Some(caps) = progress_re.captures(&line) {
                                if let Ok(pct) = caps[1].parse::<f64>() {
                                    let rem = remaining_re.captures(&line)
                                        .map(|c| c[1].to_string()).unwrap_or_default();
                                    let spd = speed_re.captures(&line)
                                        .map(|c| c[1].to_string()).unwrap_or_default();
                                    let detail = extract_progress_detail(&line);
                                    let msg = if attempt > 1 {
                                        format!("{}[Retry #{}] {}", label, attempt, detail)
                                    } else {
                                        format!("{}{}", label, detail)
                                    };
                                    let mapped = pct_base + pct * pct_span / 100.0;
                                    let _ = app.emit("progress", ProgressPayload {
                                        file_index: index, total_files,
                                        file_name: file_name.to_string(),
                                        progress: mapped, status: "processing".to_string(),
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
                write_log(&format!("RETRY file=\"{}\" {}attempt={} error: process wait failed: {}", file_name, label, attempt, e));
                tokio::time::sleep(std::time::Duration::from_secs(30)).await;
                continue;
            }
        };

        if CANCEL_FLAG.load(Ordering::SeqCst) { return false; }

        let output_valid = expected_output.exists()
            && std::fs::metadata(expected_output).map(|m| m.len() > 1000).unwrap_or(false);

        if status.success() && output_valid {
            write_log(&format!("PASS-OK file=\"{}\" {}attempts={}", file_name, label, attempt));
            return true;
        } else {
            let stderr_tail = last_stderr_lines.join("\n");
            // Docker exit 125 + a mount-source error almost always means the drive
            // holding the video (or the work dir) went away — a disconnected
            // external disk, or Docker Desktop losing that drive share.
            let hint = if status.code() == Some(125) && stderr_tail.contains("mount source path") {
                " [drive unavailable — check the drive is connected, then restart Docker Desktop]"
            } else {
                ""
            };
            let error_msg = if status.success() && !output_valid {
                format!("{}Attempt {}: output missing. Retrying in 30s...\n{}", label, attempt, stderr_tail)
            } else {
                format!("{}Attempt {}: exit code {:?}.{} Retrying in 30s...\n{}", label, attempt, status.code(), hint, stderr_tail)
            };
            write_log(&format!("RETRY file=\"{}\" {}attempt={} error: {}", file_name, label, attempt, error_msg.replace('\n', " | ")));
            let _ = app.emit("progress", ProgressPayload {
                file_index: index, total_files,
                file_name: file_name.to_string(), progress: pct_base,
                status: "processing".to_string(), message: error_msg,
                remaining: String::new(), speed: String::new(),
            });
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            let _ = std::fs::remove_file(expected_output);
        }
    }
}

/// Free bytes on the filesystem holding `path` (longest matching mount point).
fn available_space_for(path: &std::path::Path) -> Option<u64> {
    let disks = sysinfo::Disks::new_with_refreshed_list();
    let mut best: Option<(usize, u64)> = None;
    for d in disks.list() {
        let mp = d.mount_point();
        if path.starts_with(mp) {
            let len = mp.as_os_str().len();
            if best.map_or(true, |(blen, _)| len > blen) {
                best = Some((len, d.available_space()));
            }
        }
    }
    best.map(|(_, free)| free)
}

/// Pick where the big VR intermediates live. Prefer the system temp dir — it is
/// normally a fast internal SSD — and fall back to the output drive only when
/// system temp lacks room. Writing tens of GB to a slow/removable output drive
/// is both slower and riskier (an external HDD dropping mid-job kills the run).
fn pick_vr_work_parent(input_path: &std::path::Path, output_dir: &std::path::Path) -> PathBuf {
    let input_size = std::fs::metadata(input_path).map(|m| m.len()).unwrap_or(0);
    // Staged source + split halves + restored halves, with each half freed as
    // soon as it is consumed; measured peak is ~4-5x the source for 8K SBS.
    let needed = input_size.saturating_mul(5);
    let sys_tmp = std::env::temp_dir();
    match available_space_for(&sys_tmp) {
        Some(free) if free > needed => {
            write_log(&format!("VR-WORKDIR system temp (free={}GB needed={}GB)",
                free / 1_000_000_000, needed / 1_000_000_000));
            sys_tmp
        }
        other => {
            write_log(&format!("VR-WORKDIR output drive (system temp free={:?} needed={}GB)",
                other.map(|f| f / 1_000_000_000), needed / 1_000_000_000));
            output_dir.to_path_buf()
        }
    }
}

/// Copy a file in chunks, reporting progress into [pct_base, pct_base+pct_span].
/// Retries until it succeeds or the job is cancelled: the source or destination
/// may live on a removable drive that briefly drops and reappears. This runs as
/// ordinary host file I/O (never a Docker mount), which is exactly why it can
/// recover — Docker Desktop keeps a broken mount for a dropped drive until it is
/// restarted, whereas a plain Windows copy succeeds again the moment it returns.
#[allow(clippy::too_many_arguments)]
async fn copy_with_progress(
    app: &tauri::AppHandle,
    src: &std::path::Path,
    dst: &std::path::Path,
    index: usize,
    total_files: usize,
    file_name: &str,
    label: &str,
    pct_base: f64,
    pct_span: f64,
) -> Result<(), String> {
    let mut attempt = 0u32;
    loop {
        attempt += 1;
        if CANCEL_FLAG.load(Ordering::SeqCst) { return Err("cancelled".into()); }

        let app_c = app.clone();
        let src_c = src.to_path_buf();
        let dst_c = dst.to_path_buf();
        let fname = file_name.to_string();
        let lbl = label.to_string();

        let res = tokio::task::spawn_blocking(move || -> std::io::Result<()> {
            use std::io::Read;
            let mut fi = std::fs::File::open(&src_c)?;
            let total = fi.metadata()?.len().max(1);
            let mut fo = std::fs::File::create(&dst_c)?;
            let mut buf = vec![0u8; 8 * 1024 * 1024];
            let mut done: u64 = 0;
            let mut last_emit: u64 = 0;
            loop {
                if CANCEL_FLAG.load(Ordering::SeqCst) {
                    return Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "cancelled"));
                }
                let n = fi.read(&mut buf)?;
                if n == 0 { break; }
                fo.write_all(&buf[..n])?;
                done += n as u64;
                if done - last_emit >= 256 * 1024 * 1024 {
                    last_emit = done;
                    let frac = done as f64 / total as f64;
                    let _ = app_c.emit("progress", ProgressPayload {
                        file_index: index, total_files, file_name: fname.clone(),
                        progress: pct_base + frac * pct_span,
                        status: "processing".to_string(),
                        message: format!("{}{:.0}%", lbl, frac * 100.0),
                        remaining: String::new(), speed: String::new(),
                    });
                }
            }
            fo.flush()?;
            Ok(())
        }).await;

        match res {
            Ok(Ok(())) => return Ok(()),
            Ok(Err(e)) => {
                if CANCEL_FLAG.load(Ordering::SeqCst) { return Err("cancelled".into()); }
                let msg = format!("{}attempt {} failed: {} — retrying in 15s (is the drive connected?)", label, attempt, e);
                write_log(&format!("VR-COPY-RETRY src=\"{}\" {}", src.display(), msg));
                let _ = app.emit("progress", ProgressPayload {
                    file_index: index, total_files, file_name: file_name.to_string(),
                    progress: pct_base, status: "processing".to_string(), message: msg,
                    remaining: String::new(), speed: String::new(),
                });
                let _ = std::fs::remove_file(dst); // drop the partial copy
                tokio::time::sleep(std::time::Duration::from_secs(15)).await;
            }
            Err(e) => return Err(format!("{}copy task panicked: {}", label, e)),
        }
    }
}

/// VR (side-by-side) pipeline: stage the source onto the work drive → split L/R
/// → Lada each half → hstack + audio → stage the result back out.
///
/// Everything Docker touches lives in `work_dir`. The source and output drives
/// are only ever read/written by the two host-side copies, so a removable drive
/// blinking out mid-job can no longer strand a multi-hour run behind a dead
/// Docker mount — the copy simply retries once the drive is back.
#[allow(clippy::too_many_arguments)]
async fn process_vr_file(
    app: &tauri::AppHandle,
    input_path: &std::path::Path,
    output_dir: &std::path::Path,
    output_filename: &str,
    index: usize,
    total_files: usize,
    file_name: &str,
    duration_sec: f64,
    settings: &LadaSettings,
) {
    // An 8K VR job's staged source plus four halves total tens of GB, so place
    // them on whichever drive can take it: fast system temp when it has room,
    // else the output drive.
    let work_dir = pick_vr_work_parent(input_path, output_dir).join(format!("lada-vr-tmp-{}", index));
    let _ = std::fs::create_dir_all(&work_dir);
    let work_dir_docker = to_docker_volume_path(work_dir.to_str().unwrap());
    if !cfg!(windows) {
        let _ = Command::new("chmod").args(["777", &work_dir_docker]).output().await;
    }

    // Stage under a plain ASCII name so the original's spaces/brackets/non-Latin
    // characters never have to survive a round trip through docker arguments.
    let ext = input_path.extension().and_then(|e| e.to_str()).unwrap_or("mp4");
    let staged_name = format!("vr_source.{}", ext);
    let staged_src = work_dir.join(&staged_name);
    let left_in = work_dir.join("vr_left.mp4");
    let right_in = work_dir.join("vr_right.mp4");
    let left_out = work_dir.join("vr_left_out.mp4");
    let right_out = work_dir.join("vr_right_out.mp4");
    let merged = work_dir.join("vr_merged.mp4");
    let cleanup = || { let _ = std::fs::remove_dir_all(&work_dir); };

    let fail = |stage: &str, e: String| {
        write_log(&format!("VR-{}-FAIL file=\"{}\" error: {}", stage, file_name, e));
        let _ = app.emit("progress", ProgressPayload {
            file_index: index, total_files, file_name: file_name.to_string(), progress: 0.0,
            status: "error".to_string(), message: format!("VR {} failed: {}", stage.to_lowercase(), e),
            remaining: String::new(), speed: String::new(),
        });
    };

    // 1) Stage the source onto the work drive (0–5%)
    let _ = app.emit("progress", ProgressPayload {
        file_index: index, total_files, file_name: file_name.to_string(), progress: 0.0,
        status: "processing".to_string(), message: "VR detected — copying source to work drive...".to_string(),
        remaining: String::new(), speed: String::new(),
    });
    if let Err(e) = copy_with_progress(app, input_path, &staged_src, index, total_files, file_name, "VR copying in: ", 0.0, 5.0).await {
        if !CANCEL_FLAG.load(Ordering::SeqCst) { fail("COPY-IN", e); }
        cleanup();
        return;
    }

    // 2) Split into left/right halves (5–12%)
    let split_args = split_lr_args(&work_dir_docker, &work_dir_docker, &staged_name);
    if let Err(e) = run_ffmpeg_progress(app, &split_args, duration_sec, index, total_files, file_name, "VR splitting L/R: ", 5.0, 7.0).await {
        if !CANCEL_FLAG.load(Ordering::SeqCst) { fail("SPLIT", e); }
        cleanup();
        return;
    }
    if CANCEL_FLAG.load(Ordering::SeqCst) { cleanup(); return; }

    // 3) Lada on each eye (work dir mounted as input/output/tmp).
    // memory_limit=0 (unlimited) + a low clip length: each split eye is 4K, where the
    // normal (up to 180-frame) clip window needs tens of GB and OOM-kills the container
    // (exit 137 crash-loop). Capping the VR clip window keeps 4K RAM/VRAM bounded
    // (~7 GB, verified) while VR's sequential passes make uncapped memory safe.
    // Each half is deleted once restored, to bound peak disk use.
    let vr_clip = settings.max_clip_length.min(VR_MAX_CLIP_LENGTH);
    let left_args = build_lada_args(&work_dir_docker, &work_dir_docker, &work_dir_docker, "vr_left.mp4", "vr_left_out.mp4", settings, 0, vr_clip);
    if !run_lada_pass(app, &left_args, &left_out, index, total_files, file_name, "VR L: ", 12.0, 40.0).await {
        cleanup(); return; // cancelled
    }
    let _ = std::fs::remove_file(&left_in);
    let right_args = build_lada_args(&work_dir_docker, &work_dir_docker, &work_dir_docker, "vr_right.mp4", "vr_right_out.mp4", settings, 0, vr_clip);
    if !run_lada_pass(app, &right_args, &right_out, index, total_files, file_name, "VR R: ", 52.0, 40.0).await {
        cleanup(); return; // cancelled
    }
    let _ = std::fs::remove_file(&right_in);

    // 4) Merge halves back + mux the staged source's audio (92–97%)
    let _ = app.emit("progress", ProgressPayload {
        file_index: index, total_files, file_name: file_name.to_string(), progress: 92.0,
        status: "processing".to_string(), message: "VR: merging L/R + audio...".to_string(),
        remaining: String::new(), speed: String::new(),
    });
    let merge_args = merge_lr_args(&work_dir_docker, &work_dir_docker, &work_dir_docker, &staged_name, "vr_merged.mp4", settings);
    if let Err(e) = run_ffmpeg_progress(app, &merge_args, duration_sec, index, total_files, file_name, "VR merging: ", 92.0, 5.0).await {
        if !CANCEL_FLAG.load(Ordering::SeqCst) { fail("MERGE", e); }
        cleanup();
        return;
    }

    let merged_valid = merged.exists()
        && std::fs::metadata(&merged).map(|m| m.len() > 1000).unwrap_or(false);
    if !merged_valid {
        fail("MERGE", "merged output missing/invalid".to_string());
        cleanup();
        return;
    }
    // Free the halves before writing the result back out.
    let _ = std::fs::remove_file(&left_out);
    let _ = std::fs::remove_file(&right_out);
    let _ = std::fs::remove_file(&staged_src);

    // 5) Stage the finished file back to the output drive (97–100%)
    let output_file_path = output_dir.join(output_filename);
    if let Err(e) = copy_with_progress(app, &merged, &output_file_path, index, total_files, file_name, "VR copying out: ", 97.0, 3.0).await {
        if !CANCEL_FLAG.load(Ordering::SeqCst) { fail("COPY-OUT", e); }
        cleanup();
        return;
    }

    let output_valid = output_file_path.exists()
        && std::fs::metadata(&output_file_path).map(|m| m.len() > 1000).unwrap_or(false);
    cleanup(); // remove the work dir regardless

    if !output_valid {
        let _ = app.emit("progress", ProgressPayload {
            file_index: index, total_files, file_name: file_name.to_string(), progress: 0.0,
            status: "error".to_string(), message: "VR: merged output missing/invalid".to_string(),
            remaining: String::new(), speed: String::new(),
        });
        return;
    }

    let final_msg = if settings.delete_original {
        match std::fs::remove_file(input_path) {
            Ok(_) => format!("Saved (VR): {} (original deleted)", output_filename),
            Err(e) => format!("Saved (VR): {} (failed to delete original: {})", output_filename, e),
        }
    } else {
        format!("Saved (VR): {}", output_filename)
    };
    let _ = app.emit("progress", ProgressPayload {
        file_index: index, total_files, file_name: file_name.to_string(), progress: 100.0,
        status: "done".to_string(), message: final_msg,
        remaining: String::new(), speed: String::new(),
    });
    write_log(&format!("DONE(VR) file=\"{}\" output=\"{}\"", file_name, output_filename));
}

async fn process_single_file(
    app: tauri::AppHandle,
    file_path: String,
    index: usize,
    total_files: usize,
    settings: LadaSettings,
) {
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

    let input_file_name = input_path.file_name().unwrap().to_string_lossy().to_string();

    // Auto-detect side-by-side VR (no user option). 2D files exit the gate after
    // one cheap probe and take the unchanged normal path below.
    if let Some(duration_sec) = detect_sbs_vr(&input_dir_docker, &input_file_name).await {
        process_vr_file(
            &app, &input_path, &output_dir, &output_filename,
            index, total_files, &file_name, duration_sec, &settings,
        ).await;
        return;
    }

    // ---- Normal 2D path ----
    let args = build_lada_args(&input_dir_docker, &output_dir_docker, &tmp_dir_docker, &input_file_name, &output_filename, &settings, settings.memory_limit, settings.max_clip_length);
    let output_file_path = output_dir.join(&output_filename);

    if !run_lada_pass(&app, &args, &output_file_path, index, total_files, &file_name, "", 0.0, 100.0).await {
        return; // cancelled
    }

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
    write_log(&format!("DONE file=\"{}\" output=\"{}\"", file_name, output_filename));
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
