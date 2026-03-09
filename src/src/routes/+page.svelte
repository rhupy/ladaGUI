<script>
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { open } from "@tauri-apps/plugin-dialog";
  import { open as shellOpen } from "@tauri-apps/plugin-shell";
  import { onMount } from "svelte";

  let files = $state([]);
  let dockerStatus = $state("Checking...");
  let dockerOk = $state(false);
  let processing = $state(false);
  let paused = $state(false);
  let updating = $state(false);

  // Overall progress tracking
  let currentFileRemaining = $state("");
  let currentFileSpeed = $state("");
  let totalFiles = $state(0);

  // Settings
  let detectionModel = $state("v4-accurate");
  let maxClipLength = $state(300);
  let encoder = $state("hevc_nvenc");
  let crf = $state(18);
  let preset = $state("medium");
  let prefix = $state("[nm]");
  let sameDirectory = $state(true);
  let outputDirectory = $state("");
  let deleteOriginal = $state(true);
  let shutdownAfter = $state(false);
  let parallelJobs = $state(1);

  // Watermark settings
  let watermarkEnabled = $state(false);
  let watermarkText = $state("");
  let watermarkSize = $state(24);
  let watermarkPosition = $state("top-left");

  // Panel toggle: "settings" | "performance" | "log" | null
  let activePanel = $state(null);

  // i18n
  let lang = $state("en");

  // Log
  let logEntries = $state([]);
  let errorCount = $state(0);

  // Performance
  let perfData = $state({ cpuUsage: 0, gpuUsage: 0, vramUsage: 0, vramTotal: 0 });
  let perfInterval = null;

  const VIDEO_EXTENSIONS = ["mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "ts"];

  // --- i18n strings ---
  const t = $derived({
    addFiles: lang === "ko" ? "+ 파일 추가" : "+ Add Files",
    clearDone: lang === "ko" ? "완료 삭제" : "Clear Done",
    clearAll: lang === "ko" ? "전체 삭제" : "Clear All",
    settings: lang === "ko" ? "설정" : "Settings",
    performance: lang === "ko" ? "성능" : "Performance",
    log: lang === "ko" ? "로그" : "Log",
    updateLada: lang === "ko" ? "Lada 업데이트" : "Update Lada",
    updating: lang === "ko" ? "업데이트 중..." : "Updating...",
    startProcessing: lang === "ko" ? "처리 시작" : "Start Processing",
    cancel: lang === "ko" ? "취소" : "Cancel",
    pause: lang === "ko" ? "일시정지" : "Pause",
    resume: lang === "ko" ? "재개" : "Resume",
    dragDrop: lang === "ko" ? "여기에 동영상 파일을 끌어다 놓거나, '파일 추가'를 클릭하세요" : 'Drag & drop video files here, or click "Add Files"',
    files: lang === "ko" ? "파일" : "files",
    overall: lang === "ko" ? "전체" : "Overall",
    remaining: lang === "ko" ? "남은 시간" : "Remaining",
    speed: lang === "ko" ? "속도" : "Speed",
    shutdownAfterDone: lang === "ko" ? "완료 후 PC 종료" : "Shutdown after done",
    detectionModel: lang === "ko" ? "감지 모델" : "Detection Model",
    maxClipLength: lang === "ko" ? "최대 클립 길이" : "Max Clip Length",
    parallelJobsLabel: lang === "ko" ? "병렬 작업 수" : "Parallel Jobs",
    encoderLabel: lang === "ko" ? "인코더" : "Encoder",
    crfLabel: lang === "ko" ? "CRF / CQ (품질)" : "CRF / CQ (quality)",
    presetLabel: lang === "ko" ? "프리셋" : "Preset",
    prefixLabel: lang === "ko" ? "파일명 접두어" : "Filename Prefix",
    deleteOrig: lang === "ko" ? "성공 후 원본 삭제" : "Delete original after success",
    shutdownOpt: lang === "ko" ? "완료 후 PC 종료" : "Shutdown PC after completion",
    sameDir: lang === "ko" ? "같은 디렉토리에 출력" : "Output to same directory",
    outputDir: lang === "ko" ? "출력 디렉토리" : "Output Directory",
    browse: lang === "ko" ? "찾아보기" : "Browse",
    languageLabel: lang === "ko" ? "언어" : "Language",
    watermark: lang === "ko" ? "워터마크" : "Watermark",
    watermarkTextLabel: lang === "ko" ? "워터마크 텍스트" : "Watermark Text",
    watermarkSizeLabel: lang === "ko" ? "글자 크기" : "Font Size",
    watermarkPosLabel: lang === "ko" ? "위치" : "Position",
    cpuUsage: lang === "ko" ? "CPU 사용률" : "CPU Usage",
    gpuUsage: lang === "ko" ? "GPU 사용률" : "GPU Usage",
    vramUsage: lang === "ko" ? "VRAM 사용량" : "VRAM Usage",
  });

  // Tooltips
  const tooltips = $derived({
    detectionModel: lang === "ko"
      ? "v4-fast: 빠르지만 정확도 낮음\nv4-accurate: 느리지만 정확도 높음"
      : "v4-fast: Faster but less accurate\nv4-accurate: Slower but better quality",
    maxClipLength: lang === "ko"
      ? "영상을 분할 처리하는 최대 길이(초)\nVRAM 부족 시 낮추세요"
      : "Max segment length in seconds\nLower if running out of VRAM",
    parallelJobs: lang === "ko"
      ? "동시 처리 영상 수. VRAM 사용량 × N\n\nRTX 3060 12GB: 1~2개\nRTX 3080/4070: 2~3개\nRTX 4080/4090: 3~6개\nRTX 5090: 4~8개"
      : "Number of concurrent videos. VRAM usage × N\n\nRTX 3060 12GB: 1-2\nRTX 3080/4070: 2-3\nRTX 4080/4090: 3-6\nRTX 5090: 4-8",
    encoder: lang === "ko"
      ? "hevc_nvenc/h264_nvenc: GPU 인코딩 (빠름)\nlibx265/libx264: CPU 인코딩 (파일 작음)"
      : "hevc_nvenc/h264_nvenc: GPU encoding (fast)\nlibx265/libx264: CPU encoding (smaller file)",
    crf: lang === "ko"
      ? "0~51. 낮을수록 고품질, 파일 큼\n권장: 18~23"
      : "0-51. Lower = better quality, larger file\nRecommended: 18-23",
    preset: lang === "ko"
      ? "인코딩 속도. ultrafast가 가장 빠르지만\n파일 크기가 커짐"
      : "Encoding speed. ultrafast is fastest\nbut produces larger files",
    watermark: lang === "ko"
      ? "출력 영상에 반투명 워터마크 텍스트 추가"
      : "Add semi-transparent watermark text to output",
  });

  function getSettingsObj() {
    return {
      detection_model: detectionModel,
      max_clip_length: maxClipLength,
      encoder,
      crf,
      preset,
      prefix,
      same_directory: sameDirectory,
      output_directory: outputDirectory,
      delete_original: deleteOriginal,
      shutdown_after: shutdownAfter,
      parallel_jobs: parallelJobs,
    };
  }

  function addFilePaths(paths) {
    for (const path of paths) {
      const ext = path.split(".").pop()?.toLowerCase();
      if (!ext || !VIDEO_EXTENSIONS.includes(ext)) continue;
      const name = path.split(/[/\\]/).pop();
      if (!files.find((f) => f.path === path)) {
        files = [
          ...files,
          { path, name, progress: 0, status: "pending", message: "" },
        ];
      }
    }
  }

  function addLogEntry(msg, type = "info") {
    const now = new Date().toLocaleTimeString();
    logEntries = [...logEntries, { time: now, msg, type }];
    if (type === "error") errorCount++;
  }

  async function persistSettings() {
    try {
      await invoke("save_settings", { settings: getSettingsObj() });
    } catch (e) {
      console.error("Failed to save settings:", e);
    }
  }

  onMount(async () => {
    // Load saved settings
    try {
      const saved = await invoke("load_settings");
      if (saved) {
        detectionModel = saved.detection_model ?? detectionModel;
        maxClipLength = saved.max_clip_length ?? maxClipLength;
        encoder = saved.encoder ?? encoder;
        crf = saved.crf ?? crf;
        preset = saved.preset ?? preset;
        prefix = saved.prefix ?? prefix;
        sameDirectory = saved.same_directory ?? sameDirectory;
        outputDirectory = saved.output_directory ?? outputDirectory;
        deleteOriginal = saved.delete_original ?? deleteOriginal;
        shutdownAfter = saved.shutdown_after ?? shutdownAfter;
        parallelJobs = saved.parallel_jobs ?? parallelJobs;
      }
    } catch (e) {
      console.log("No saved settings found");
    }

    // Load saved language
    const savedLang = localStorage.getItem("lada-gui-lang");
    if (savedLang) lang = savedLang;

    // Load saved logs
    const savedLogs = localStorage.getItem("lada-gui-logs");
    if (savedLogs) {
      try {
        const parsed = JSON.parse(savedLogs);
        logEntries = parsed;
        errorCount = parsed.filter((e) => e.type === "error").length;
      } catch (e) {}
    }

    await checkDocker();

    document.addEventListener("dragover", (e) => {
      e.preventDefault();
      e.stopPropagation();
      dragging = true;
    });
    document.addEventListener("drop", (e) => {
      e.preventDefault();
      dragging = false;
    });
    document.addEventListener("dragleave", (e) => {
      if (e.relatedTarget === null) dragging = false;
    });

    listen("progress", (event) => {
      const p = event.payload;
      const idx = pendingIndices?.[p.file_index] ?? p.file_index;
      if (idx < files.length) {
        files[idx] = {
          ...files[idx],
          progress: p.progress,
          status: p.status,
          message: p.message,
        };
      }
      totalFiles = p.total_files || files.length;
      if (p.remaining) currentFileRemaining = p.remaining;
      if (p.speed) currentFileSpeed = p.speed;

      // Log events
      if (p.status === "done") {
        addLogEntry(`[DONE] ${p.file_name}: ${p.message}`);
        currentFileRemaining = "";
        currentFileSpeed = "";
      }
      if (p.status === "error") {
        addLogEntry(`[ERROR] ${p.file_name}: ${p.message}`, "error");
        currentFileRemaining = "";
        currentFileSpeed = "";
      }
      if (p.message && p.message.startsWith("Attempt") && p.message.includes("Retrying")) {
        addLogEntry(`[RETRY] ${p.file_name}: ${p.message}`, "error");
      }

      // Save logs periodically
      saveLogs();
    });

    listen("files-dropped", (event) => {
      addFilePaths(event.payload.paths);
      dragging = false;
    });

    listen("tauri://drag-drop", (event) => {
      dragging = false;
      if (event.payload?.paths) {
        addFilePaths(event.payload.paths);
      }
    });
    listen("tauri://drag-enter", () => { dragging = true; });
    listen("tauri://drag-leave", () => { dragging = false; });
  });

  function saveLogs() {
    try {
      // Keep last 500 entries
      const trimmed = logEntries.slice(-500);
      localStorage.setItem("lada-gui-logs", JSON.stringify(trimmed));
    } catch (e) {}
  }

  async function checkDocker() {
    try {
      const result = await invoke("check_docker");
      dockerStatus = result;
      dockerOk = true;
    } catch (e) {
      dockerStatus = e;
      dockerOk = false;
    }
  }

  async function addFiles() {
    const selected = await open({
      multiple: true,
      filters: [{ name: "Video", extensions: VIDEO_EXTENSIONS }],
    });
    if (selected) {
      const paths = Array.isArray(selected) ? selected : [selected];
      addFilePaths(paths.map((p) => (typeof p === "string" ? p : p.path)));
    }
  }

  function removeFile(index) {
    const file = files[index];
    if (file.status === "processing") {
      // TODO: cancel single file
    }
    files = files.filter((_, i) => i !== index);
  }

  function clearDone() {
    files = files.filter((f) => f.status === "pending" || f.status === "processing");
  }

  function clearAll() {
    if (processing) {
      cancelProcessing();
    }
    files = [];
  }

  async function startProcessing() {
    if (files.length === 0 || processing) return;

    files = files.map((f) =>
      f.status === "pending" || f.status === "error" || f.status === "cancelled"
        ? { ...f, progress: 0, status: "pending", message: "" }
        : f
    );

    processing = true;
    paused = false;
    errorCount = 0;

    pendingIndices = files
      .map((f, i) => (f.status === "pending" ? i : -1))
      .filter((i) => i !== -1);
    const pendingFiles = pendingIndices.map((i) => files[i].path);

    addLogEntry(`--- ${lang === "ko" ? "작업 시작" : "Processing started"}: ${pendingFiles.length} ${t.files} (jobs: ${parallelJobs}) ---`);

    try {
      await invoke("process_files", {
        files: pendingFiles,
        settings: getSettingsObj(),
      });
    } catch (e) {
      console.error("Processing error:", e);
      addLogEntry(`[ERROR] Processing failed: ${e}`, "error");
    }
    processing = false;
    paused = false;
    addLogEntry(`--- ${lang === "ko" ? "작업 완료" : "Processing finished"} ---`);
    saveLogs();
  }

  async function cancelProcessing() {
    try {
      await invoke("cancel_processing");
      addLogEntry(`[CANCEL] ${lang === "ko" ? "사용자가 취소함" : "Cancelled by user"}`);
    } catch (e) {
      console.error("Cancel error:", e);
    }
  }

  async function togglePause() {
    try {
      if (paused) {
        await invoke("resume_processing");
        paused = false;
        addLogEntry(`[RESUME] ${lang === "ko" ? "재개됨" : "Resumed"}`);
      } else {
        await invoke("pause_processing");
        paused = true;
        addLogEntry(`[PAUSE] ${lang === "ko" ? "일시정지됨" : "Paused"}`);
      }
    } catch (e) {
      console.error("Pause/resume error:", e);
    }
  }

  async function updateLada() {
    updating = true;
    try {
      await invoke("update_lada");
      dockerStatus += " (Lada updated!)";
    } catch (e) {
      alert("Update failed: " + e);
    }
    updating = false;
  }

  async function selectOutputDir() {
    const selected = await open({ directory: true });
    if (selected) {
      outputDirectory = typeof selected === "string" ? selected : selected.path;
    }
  }

  function togglePanel(panel) {
    activePanel = activePanel === panel ? null : panel;
  }

  function statusIcon(status) {
    if (status === "done") return "\u2713";
    if (status === "error") return "\u2717";
    if (status === "cancelled") return "\u25CB";
    if (status === "processing") return "\u25B6";
    return "\u2022";
  }

  function statusColor(status) {
    if (status === "done") return "#4caf50";
    if (status === "error") return "#f44336";
    if (status === "cancelled") return "#ff9800";
    if (status === "processing") return "#2196f3";
    return "#888";
  }

  function setLang(newLang) {
    lang = newLang;
    localStorage.setItem("lada-gui-lang", newLang);
  }

  let dragging = $state(false);
  let pendingIndices = $state(null);
</script>

<main>
  <header>
    <div class="header-left">
      <h1>Lada GUI</h1>
      <a
        class="discord-link"
        href="#"
        onclick={(e) => { e.preventDefault(); shellOpen("https://discord.gg/px4vBjTUBg"); }}
        title="Discord"
      >
        <svg width="20" height="16" viewBox="0 0 71 55" fill="currentColor">
          <path d="M60.1 4.9A58.5 58.5 0 0045.4.2a.2.2 0 00-.2.1 40.8 40.8 0 00-1.8 3.7 54 54 0 00-16.2 0A37 37 0 0025.4.3a.2.2 0 00-.2-.1A58.4 58.4 0 0010.5 5a.2.2 0 00-.1 0C1.5 18.7-.9 32 .3 45.2a.2.2 0 00.1.1 58.7 58.7 0 0017.7 9 .2.2 0 00.3-.1 42 42 0 003.6-5.9.2.2 0 00-.1-.3 38.7 38.7 0 01-5.5-2.7.2.2 0 01 0-.4l1.1-.9a.2.2 0 01.2 0 41.9 41.9 0 0035.6 0 .2.2 0 01.2 0l1.1.9a.2.2 0 010 .3 36.4 36.4 0 01-5.5 2.7.2.2 0 00-.1.4 47.2 47.2 0 003.6 5.9.2.2 0 00.3 0A58.5 58.5 0 0070.3 45.3a.2.2 0 00.1-.2c1.4-15-2.3-28-9.8-39.6a.2.2 0 00-.1 0zM23.7 37.1c-3.4 0-6.2-3.1-6.2-7s2.7-7 6.2-7 6.3 3.2 6.2 7-2.8 7-6.2 7zm22.9 0c-3.4 0-6.2-3.1-6.2-7s2.7-7 6.2-7 6.3 3.2 6.2 7-2.7 7-6.2 7z"/>
        </svg>
      </a>
    </div>
    <div class="header-right">
      <div class="lang-toggle">
        <button class="lang-btn" class:active={lang === "en"} onclick={() => setLang("en")}>EN</button>
        <button class="lang-btn" class:active={lang === "ko"} onclick={() => setLang("ko")}>KO</button>
      </div>
      <div class="status" class:ok={dockerOk} class:err={!dockerOk}>
        {dockerStatus}
      </div>
    </div>
  </header>

  <div class="toolbar">
    <button onclick={addFiles}>{t.addFiles}</button>
    <button onclick={clearDone}>{t.clearDone}</button>
    <button onclick={clearAll}>{t.clearAll}</button>
    <button class="panel-btn" class:active={activePanel === "settings"} onclick={() => togglePanel("settings")}>
      {t.settings}
    </button>
    <button class="panel-btn" class:active={activePanel === "performance"} onclick={() => togglePanel("performance")}>
      {t.performance}
    </button>
    <button class="panel-btn" class:active={activePanel === "log"} onclick={() => togglePanel("log")}>
      {t.log}
      {#if errorCount > 0}
        <span class="error-badge">{errorCount}</span>
      {/if}
    </button>
    <div class="spacer"></div>
    <button class="update-btn" onclick={updateLada} disabled={updating || processing}>
      {updating ? t.updating : t.updateLada}
    </button>
  </div>

  {#if activePanel === "settings"}
    <div class="settings-panel">
      <div class="setting-row">
        <label>{t.detectionModel}</label>
        <select bind:value={detectionModel} disabled={processing} onchange={persistSettings}>
          <option value="v4-fast">v4-fast</option>
          <option value="v4-accurate">v4-accurate</option>
        </select>
        <span class="tooltip-icon" title={tooltips.detectionModel}>?</span>
      </div>
      <div class="setting-row">
        <label>{t.maxClipLength}</label>
        <input type="number" bind:value={maxClipLength} min="30" max="600" disabled={processing} onchange={persistSettings} />
        <span class="tooltip-icon" title={tooltips.maxClipLength}>?</span>
      </div>
      <div class="setting-row">
        <label>{t.parallelJobsLabel}</label>
        <div class="number-spinner">
          <button class="spin-btn" onclick={() => { if (parallelJobs > 1) { parallelJobs--; persistSettings(); } }} disabled={processing}>-</button>
          <input
            type="number"
            bind:value={parallelJobs}
            min="1"
            max="99"
            disabled={processing}
            onchange={(e) => {
              let v = parseInt(e.target.value) || 1;
              if (v < 1) v = 1;
              if (v > 99) v = 99;
              parallelJobs = v;
              persistSettings();
            }}
          />
          <button class="spin-btn" onclick={() => { if (parallelJobs < 99) { parallelJobs++; persistSettings(); } }} disabled={processing}>+</button>
        </div>
        <span class="tooltip-icon" title={tooltips.parallelJobs}>?</span>
      </div>
      <div class="setting-row">
        <label>{t.encoderLabel}</label>
        <select bind:value={encoder} disabled={processing} onchange={persistSettings}>
          <option value="hevc_nvenc">hevc_nvenc (GPU)</option>
          <option value="h264_nvenc">h264_nvenc (GPU)</option>
          <option value="libx265">libx265 (CPU)</option>
          <option value="libx264">libx264 (CPU)</option>
        </select>
        <span class="tooltip-icon" title={tooltips.encoder}>?</span>
      </div>
      <div class="setting-row">
        <label>{t.crfLabel}</label>
        <input type="number" bind:value={crf} min="0" max="51" disabled={processing} onchange={persistSettings} />
        <span class="tooltip-icon" title={tooltips.crf}>?</span>
      </div>
      <div class="setting-row">
        <label>{t.presetLabel}</label>
        <select bind:value={preset} disabled={processing} onchange={persistSettings}>
          <option value="ultrafast">ultrafast</option>
          <option value="fast">fast</option>
          <option value="medium">medium</option>
          <option value="slow">slow</option>
          <option value="veryslow">veryslow</option>
        </select>
        <span class="tooltip-icon" title={tooltips.preset}>?</span>
      </div>
      <div class="setting-row">
        <label>{t.prefixLabel}</label>
        <input type="text" bind:value={prefix} disabled={processing} onchange={persistSettings} />
      </div>
      <div class="setting-row">
        <label>
          <input type="checkbox" bind:checked={deleteOriginal} disabled={processing} onchange={persistSettings} />
          {t.deleteOrig}
        </label>
      </div>
      <div class="setting-row">
        <label>
          <input type="checkbox" bind:checked={shutdownAfter} onchange={persistSettings} />
          {t.shutdownOpt}
        </label>
      </div>
      <div class="setting-row">
        <label>
          <input type="checkbox" bind:checked={sameDirectory} disabled={processing} onchange={persistSettings} />
          {t.sameDir}
        </label>
      </div>
      {#if !sameDirectory}
        <div class="setting-row">
          <label>{t.outputDir}</label>
          <div class="dir-select">
            <input type="text" bind:value={outputDirectory} placeholder="..." disabled={processing} onchange={persistSettings} />
            <button onclick={selectOutputDir} disabled={processing}>{t.browse}</button>
          </div>
        </div>
      {/if}
      <div class="setting-row">
        <label>
          <input type="checkbox" bind:checked={watermarkEnabled} disabled={processing} onchange={persistSettings} />
          {t.watermark}
        </label>
        <span class="tooltip-icon" title={tooltips.watermark}>?</span>
      </div>
      {#if watermarkEnabled}
        <div class="setting-row sub-setting">
          <label>{t.watermarkTextLabel}</label>
          <input type="text" bind:value={watermarkText} placeholder="Your text..." disabled={processing} />
        </div>
        <div class="setting-row sub-setting">
          <label>{t.watermarkSizeLabel}</label>
          <input type="number" bind:value={watermarkSize} min="8" max="120" disabled={processing} />
        </div>
        <div class="setting-row sub-setting">
          <label>{t.watermarkPosLabel}</label>
          <select bind:value={watermarkPosition} disabled={processing}>
            <option value="top-left">{lang === "ko" ? "좌측 상단" : "Top Left"}</option>
            <option value="top-right">{lang === "ko" ? "우측 상단" : "Top Right"}</option>
            <option value="bottom-left">{lang === "ko" ? "좌측 하단" : "Bottom Left"}</option>
            <option value="bottom-right">{lang === "ko" ? "우측 하단" : "Bottom Right"}</option>
            <option value="center">{lang === "ko" ? "중앙" : "Center"}</option>
          </select>
        </div>
      {/if}
      <div class="setting-row">
        <label>{t.languageLabel}</label>
        <select value={lang} onchange={(e) => setLang(e.target.value)}>
          <option value="en">English</option>
          <option value="ko">한국어</option>
        </select>
      </div>
    </div>
  {/if}

  {#if activePanel === "performance"}
    <div class="settings-panel perf-panel">
      <div class="perf-row">
        <span class="perf-label">{t.cpuUsage}</span>
        <div class="perf-bar-container">
          <div class="perf-bar cpu-bar" style="width: {perfData.cpuUsage}%"></div>
          <span class="perf-value">{perfData.cpuUsage}%</span>
        </div>
      </div>
      <div class="perf-row">
        <span class="perf-label">{t.gpuUsage}</span>
        <div class="perf-bar-container">
          <div class="perf-bar gpu-bar" style="width: {perfData.gpuUsage}%"></div>
          <span class="perf-value">{perfData.gpuUsage}%</span>
        </div>
      </div>
      <div class="perf-row">
        <span class="perf-label">{t.vramUsage}</span>
        <div class="perf-bar-container">
          <div class="perf-bar vram-bar" style="width: {perfData.vramTotal > 0 ? (perfData.vramUsage / perfData.vramTotal * 100) : 0}%"></div>
          <span class="perf-value">{perfData.vramUsage} / {perfData.vramTotal} MB</span>
        </div>
      </div>
      <p class="perf-note">{lang === "ko" ? "성능 모니터링은 향후 업데이트에서 실시간으로 제공됩니다" : "Real-time monitoring coming in a future update"}</p>
    </div>
  {/if}

  {#if activePanel === "log"}
    <div class="settings-panel log-panel">
      <div class="log-content" id="log-scroll">
        {#if logEntries.length === 0}
          <div class="log-empty">{lang === "ko" ? "로그가 없습니다" : "No log entries"}</div>
        {:else}
          {#each logEntries as entry}
            <div class="log-entry" class:log-error={entry.type === "error"}>
              <span class="log-time">{entry.time}</span>
              <span class="log-msg">{entry.msg}</span>
            </div>
          {/each}
        {/if}
      </div>
      <div class="log-actions">
        <button onclick={() => { logEntries = []; errorCount = 0; saveLogs(); }}>
          {lang === "ko" ? "로그 삭제" : "Clear Logs"}
        </button>
      </div>
    </div>
  {/if}

  <div
    class="file-list"
    class:dragging
    role="list"
  >
    {#if files.length === 0}
      <div class="empty-state">{t.dragDrop}</div>
    {:else}
      {#each files as file, index}
        <div class="file-item" role="listitem">
          <div class="file-header">
            <span class="status-icon" style="color: {statusColor(file.status)}">
              {statusIcon(file.status)}
            </span>
            <span class="file-name" title={file.path}>{file.name}</span>
            {#if file.status !== "processing"}
              <button class="remove-btn" onclick={() => removeFile(index)}>×</button>
            {/if}
          </div>
          {#if file.status === "processing" || file.status === "done"}
            <div class="progress-bar-container">
              <div
                class="progress-bar"
                class:done={file.status === "done"}
                style="width: {file.progress}%"
              ></div>
              <span class="progress-text">{Math.round(file.progress)}%</span>
            </div>
            {#if file.status === "processing" && currentFileRemaining && currentFileRemaining !== "?"}
              <div class="progress-meta">
                <span>{t.remaining}: {currentFileRemaining}</span>
                {#if currentFileSpeed && currentFileSpeed !== "?"}
                  <span>{t.speed}: {currentFileSpeed}</span>
                {/if}
              </div>
            {/if}
          {/if}
          {#if file.message}
            <div class="file-message">{file.message}</div>
          {/if}
        </div>
      {/each}
    {/if}
  </div>

  {#if processing}
    {@const doneCount = files.filter((f) => f.status === "done").length}
    {@const processingFiles = files.filter((f) => f.status === "processing")}
    {@const avgProgress = processingFiles.length > 0 ? processingFiles.reduce((a, f) => a + f.progress, 0) / processingFiles.length : 0}
    {@const overallPct = totalFiles > 0 ? ((doneCount + processingFiles.length * avgProgress / 100) / totalFiles) * 100 : 0}
    <div class="overall-progress">
      <div class="overall-header">
        <span>{t.overall}: {doneCount}/{totalFiles} {t.files}</span>
        <span class="overall-stats">
          {#if currentFileRemaining && currentFileRemaining !== "?"}
            {t.remaining}: {currentFileRemaining}
          {/if}
          {#if currentFileSpeed && currentFileSpeed !== "?"}
            &nbsp;| {t.speed}: {currentFileSpeed}
          {/if}
        </span>
      </div>
      <div class="progress-bar-container overall-bar">
        <div class="progress-bar" style="width: {overallPct}%"></div>
        <span class="progress-text">{Math.round(overallPct)}%</span>
      </div>
    </div>
  {/if}

  <footer>
    {#if processing}
      <div class="footer-buttons">
        <button class="pause-btn" onclick={togglePause}>
          {paused ? t.resume : t.pause}
        </button>
        <button class="cancel-btn" onclick={cancelProcessing}>{t.cancel}</button>
      </div>
    {:else}
      <button
        class="start-btn"
        onclick={startProcessing}
        disabled={files.length === 0 || !dockerOk}
      >
        {t.startProcessing} ({files.filter((f) => f.status === "pending" || f.status === "error" || f.status === "cancelled").length} {t.files})
      </button>
    {/if}
    <div class="footer-right">
      <label class="shutdown-check">
        <input type="checkbox" bind:checked={shutdownAfter} onchange={persistSettings} />
        {t.shutdownAfterDone}
      </label>
    </div>
  </footer>
</main>

<style>
  :global(body) {
    margin: 0;
    padding: 0;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
    background: #1a1a2e;
    color: #eee;
  }

  /* Custom scrollbar */
  :global(*::-webkit-scrollbar) {
    width: 8px;
    height: 8px;
  }
  :global(*::-webkit-scrollbar-track) {
    background: #0f0f23;
    border-radius: 4px;
  }
  :global(*::-webkit-scrollbar-thumb) {
    background: #333;
    border-radius: 4px;
  }
  :global(*::-webkit-scrollbar-thumb:hover) {
    background: #555;
  }

  main {
    display: flex;
    flex-direction: column;
    height: 100vh;
    padding: 16px;
    box-sizing: border-box;
    gap: 12px;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-shrink: 0;
  }

  .header-left {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .header-right {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  h1 {
    margin: 0;
    font-size: 1.4em;
    color: #e94560;
  }

  .discord-link {
    color: #7289da;
    display: flex;
    align-items: center;
    text-decoration: none;
    transition: color 0.2s;
  }
  .discord-link:hover {
    color: #99aab5;
  }

  .lang-toggle {
    display: flex;
    gap: 2px;
  }
  .lang-btn {
    padding: 2px 8px;
    font-size: 0.7em;
    border: 1px solid #444;
    background: #16213e;
    color: #888;
    cursor: pointer;
    border-radius: 3px;
  }
  .lang-btn.active {
    background: #e94560;
    color: #fff;
    border-color: #e94560;
  }

  .status {
    font-size: 0.8em;
    padding: 4px 10px;
    border-radius: 4px;
  }
  .status.ok {
    background: #1b4332;
    color: #95d5b2;
  }
  .status.err {
    background: #4a1525;
    color: #f4978e;
  }

  .toolbar {
    display: flex;
    gap: 6px;
    flex-shrink: 0;
    flex-wrap: wrap;
  }
  .spacer {
    flex: 1;
  }

  button {
    padding: 5px 12px;
    border: 1px solid #333;
    border-radius: 6px;
    background: #16213e;
    color: #eee;
    cursor: pointer;
    font-size: 0.82em;
    transition: background 0.15s;
  }
  button:hover:not(:disabled) {
    background: #0f3460;
  }
  button:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .update-btn {
    background: #1b3a4b;
    border-color: #2a6f97;
  }
  .update-btn:hover:not(:disabled) {
    background: #2a6f97;
  }

  .panel-btn {
    background: #2d2d44;
  }
  .panel-btn.active {
    background: #0f3460;
    border-color: #2196f3;
  }

  .error-badge {
    background: #f44336;
    color: #fff;
    border-radius: 10px;
    padding: 0 5px;
    font-size: 0.75em;
    margin-left: 4px;
    min-width: 16px;
    text-align: center;
    display: inline-block;
  }

  .settings-panel {
    background: #16213e;
    border: 1px solid #333;
    border-radius: 8px;
    padding: 12px 16px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    flex-shrink: 0;
    max-height: 320px;
    overflow-y: auto;
  }

  .setting-row {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 0.85em;
  }
  .setting-row label {
    min-width: 140px;
    color: #aaa;
  }
  .setting-row input[type="number"],
  .setting-row input[type="text"],
  .setting-row select {
    padding: 4px 8px;
    border: 1px solid #444;
    border-radius: 4px;
    background: #0f0f23;
    color: #eee;
    font-size: 0.9em;
  }
  .setting-row input[type="checkbox"] {
    margin-right: 6px;
  }
  .sub-setting {
    padding-left: 20px;
  }
  .dir-select {
    display: flex;
    gap: 6px;
    flex: 1;
  }
  .dir-select input {
    flex: 1;
  }

  .tooltip-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: #333;
    color: #aaa;
    font-size: 0.7em;
    font-weight: bold;
    cursor: help;
    flex-shrink: 0;
    white-space: pre-line;
  }

  .number-spinner {
    display: flex;
    align-items: center;
    gap: 0;
  }
  .number-spinner input {
    width: 50px;
    text-align: center;
    border-radius: 0;
    border-left: none;
    border-right: none;
    -moz-appearance: textfield;
  }
  .number-spinner input::-webkit-outer-spin-button,
  .number-spinner input::-webkit-inner-spin-button {
    -webkit-appearance: none;
    margin: 0;
  }
  .spin-btn {
    padding: 4px 10px;
    border-radius: 0;
    font-size: 0.9em;
    font-weight: bold;
    border: 1px solid #444;
    background: #0f0f23;
    color: #eee;
  }
  .spin-btn:first-child {
    border-radius: 4px 0 0 4px;
  }
  .spin-btn:last-child {
    border-radius: 0 4px 4px 0;
  }

  /* Log panel */
  .log-panel {
    max-height: 200px;
  }
  .log-content {
    flex: 1;
    overflow-y: auto;
    font-family: "Consolas", "Monaco", monospace;
    font-size: 0.75em;
    line-height: 1.6;
    min-height: 80px;
  }
  .log-empty {
    color: #555;
    text-align: center;
    padding: 20px;
  }
  .log-entry {
    padding: 1px 4px;
    border-bottom: 1px solid #1a1a2e;
  }
  .log-error {
    color: #f44336;
    background: rgba(244, 67, 54, 0.05);
  }
  .log-time {
    color: #666;
    margin-right: 8px;
  }
  .log-msg {
    color: #ccc;
  }
  .log-error .log-msg {
    color: #f44336;
  }
  .log-actions {
    display: flex;
    justify-content: flex-end;
    margin-top: 6px;
  }

  /* Performance panel */
  .perf-panel {
    max-height: 200px;
  }
  .perf-row {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 0.85em;
  }
  .perf-label {
    min-width: 100px;
    color: #aaa;
  }
  .perf-bar-container {
    flex: 1;
    height: 20px;
    background: #0f0f23;
    border-radius: 4px;
    position: relative;
    overflow: hidden;
  }
  .perf-bar {
    height: 100%;
    border-radius: 4px;
    transition: width 0.5s;
  }
  .cpu-bar { background: #2196f3; }
  .gpu-bar { background: #e94560; }
  .vram-bar { background: #ff9800; }
  .perf-value {
    position: absolute;
    right: 8px;
    top: 0;
    line-height: 20px;
    font-size: 0.8em;
    color: #eee;
    font-variant-numeric: tabular-nums;
  }
  .perf-note {
    font-size: 0.75em;
    color: #555;
    text-align: center;
    margin: 4px 0 0;
  }

  .file-list {
    flex: 1;
    overflow-y: auto;
    border: 2px dashed #333;
    border-radius: 8px;
    padding: 8px;
    min-height: 120px;
    transition: border-color 0.2s, background 0.2s;
  }
  .file-list.dragging {
    border-color: #e94560;
    background: rgba(233, 69, 96, 0.05);
  }

  .empty-state {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: #555;
    font-size: 0.95em;
  }

  .file-item {
    padding: 8px 10px;
    margin-bottom: 4px;
    background: #16213e;
    border-radius: 6px;
  }

  .file-header {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .status-icon {
    font-size: 0.9em;
    flex-shrink: 0;
  }

  .file-name {
    flex: 1;
    font-size: 0.85em;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .remove-btn {
    padding: 0 6px;
    border: none;
    background: transparent;
    color: #666;
    font-size: 1.2em;
    cursor: pointer;
  }
  .remove-btn:hover {
    color: #f44336;
    background: transparent;
  }

  .progress-bar-container {
    position: relative;
    height: 18px;
    background: #0f0f23;
    border-radius: 4px;
    margin-top: 6px;
    overflow: hidden;
  }
  .progress-bar {
    height: 100%;
    background: linear-gradient(90deg, #e94560, #0f3460);
    border-radius: 4px;
    transition: width 0.3s ease;
  }
  .progress-bar.done {
    background: #4caf50;
  }
  .progress-text {
    position: absolute;
    top: 0;
    right: 8px;
    line-height: 18px;
    font-size: 0.75em;
    color: #eee;
    font-variant-numeric: tabular-nums;
  }

  .progress-meta {
    display: flex;
    justify-content: flex-end;
    gap: 16px;
    font-size: 0.72em;
    color: #888;
    margin-top: 3px;
    font-variant-numeric: tabular-nums;
  }

  .file-message {
    font-size: 0.75em;
    color: #888;
    margin-top: 4px;
    white-space: pre-wrap;
    word-break: break-all;
    max-height: 60px;
    overflow-y: auto;
  }

  .overall-progress {
    flex-shrink: 0;
    background: #16213e;
    border: 1px solid #333;
    border-radius: 8px;
    padding: 10px 14px;
  }
  .overall-header {
    display: flex;
    justify-content: space-between;
    font-size: 0.85em;
    margin-bottom: 6px;
    color: #ccc;
  }
  .overall-stats {
    color: #95d5b2;
    font-size: 0.8em;
    font-variant-numeric: tabular-nums;
  }
  .overall-bar {
    height: 22px;
  }

  footer {
    flex-shrink: 0;
    display: flex;
    justify-content: center;
    align-items: center;
    gap: 16px;
  }

  .footer-buttons {
    display: flex;
    gap: 10px;
  }

  .footer-right {
    position: absolute;
    right: 16px;
  }

  .shutdown-check {
    font-size: 0.8em;
    color: #888;
    display: flex;
    align-items: center;
    gap: 4px;
    cursor: pointer;
    user-select: none;
  }
  .shutdown-check input {
    margin: 0;
  }

  .start-btn {
    background: #e94560;
    border-color: #e94560;
    color: #fff;
    font-size: 1em;
    padding: 10px 40px;
    border-radius: 8px;
    font-weight: 600;
  }
  .start-btn:hover:not(:disabled) {
    background: #c73651;
  }

  .cancel-btn {
    background: #f44336;
    border-color: #f44336;
    color: #fff;
    font-size: 1em;
    padding: 10px 30px;
    border-radius: 8px;
    font-weight: 600;
  }
  .cancel-btn:hover {
    background: #d32f2f;
  }

  .pause-btn {
    background: #ff9800;
    border-color: #ff9800;
    color: #fff;
    font-size: 1em;
    padding: 10px 30px;
    border-radius: 8px;
    font-weight: 600;
  }
  .pause-btn:hover {
    background: #e68900;
  }
</style>
