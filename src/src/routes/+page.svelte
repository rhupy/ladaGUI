<script>
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { open } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";

  let files = $state([]);
  let dockerStatus = $state("Checking...");
  let dockerOk = $state(false);
  let processing = $state(false);
  let updating = $state(false);

  // Overall progress tracking
  let currentFileRemaining = $state("");
  let currentFileSpeed = $state("");
  let totalFiles = $state(0);

  // Settings
  let detectionModel = $state("v4-accurate");
  let maxClipLength = $state(300);
  let encoder = $state("libx265");
  let crf = $state(18);
  let preset = $state("medium");
  let prefix = $state("[nm]");
  let sameDirectory = $state(true);
  let outputDirectory = $state("");
  let deleteOriginal = $state(true);

  // Settings panel toggle
  let showSettings = $state(false);

  const VIDEO_EXTENSIONS = ["mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "ts"];

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

  onMount(async () => {
    await checkDocker();

    // Prevent default drag behavior on the entire document
    // This is REQUIRED for WebView2 to accept file drops
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
      // Only set false when leaving the window entirely
      if (e.relatedTarget === null) dragging = false;
    });

    listen("progress", (event) => {
      const p = event.payload;
      const idx = p.file_index;
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
      if (p.status === "done" || p.status === "error") {
        currentFileRemaining = "";
        currentFileSpeed = "";
      }
    });

    listen("files-dropped", (event) => {
      addFilePaths(event.payload.paths);
      dragging = false;
    });

    // tauri://drag-drop contains paths directly from OS native drag
    listen("tauri://drag-drop", (event) => {
      dragging = false;
      if (event.payload?.paths) {
        addFilePaths(event.payload.paths);
      }
    });
    listen("tauri://drag-enter", () => { dragging = true; });
    listen("tauri://drag-leave", () => { dragging = false; });
  });

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
      filters: [
        {
          name: "Video",
          extensions: VIDEO_EXTENSIONS,
        },
      ],
    });
    if (selected) {
      const paths = Array.isArray(selected) ? selected : [selected];
      addFilePaths(paths.map((p) => (typeof p === "string" ? p : p.path)));
    }
  }

  function removeFile(index) {
    files = files.filter((_, i) => i !== index);
  }

  function clearDone() {
    files = files.filter((f) => f.status !== "done" && f.status !== "error");
  }

  async function startProcessing() {
    if (files.length === 0 || processing) return;

    // Reset pending files
    files = files.map((f) =>
      f.status === "pending" || f.status === "error"
        ? { ...f, progress: 0, status: "pending", message: "" }
        : f
    );

    processing = true;
    const pendingFiles = files
      .filter((f) => f.status === "pending")
      .map((f) => f.path);

    try {
      await invoke("process_files", {
        files: pendingFiles,
        settings: {
          detection_model: detectionModel,
          max_clip_length: maxClipLength,
          encoder: encoder,
          crf: crf,
          preset: preset,
          prefix: prefix,
          same_directory: sameDirectory,
          output_directory: outputDirectory,
          delete_original: deleteOriginal,
        },
      });
    } catch (e) {
      console.error("Processing error:", e);
    }
    processing = false;
  }

  async function cancelProcessing() {
    try {
      await invoke("cancel_processing");
    } catch (e) {
      console.error("Cancel error:", e);
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

  let dragging = $state(false);
</script>

<main>
  <header>
    <h1>Lada GUI</h1>
    <div class="status" class:ok={dockerOk} class:err={!dockerOk}>
      {dockerStatus}
    </div>
  </header>

  <div class="toolbar">
    <button onclick={addFiles} disabled={processing}>+ Add Files</button>
    <button onclick={clearDone} disabled={processing}>Clear Done</button>
    <button class="settings-btn" onclick={() => (showSettings = !showSettings)}>
      {showSettings ? "Hide Settings" : "Settings"}
    </button>
    <div class="spacer"></div>
    <button class="update-btn" onclick={updateLada} disabled={updating || processing}>
      {updating ? "Updating..." : "Update Lada"}
    </button>
  </div>

  {#if showSettings}
    <div class="settings-panel">
      <div class="setting-row">
        <label>Detection Model</label>
        <select bind:value={detectionModel} disabled={processing}>
          <option value="v4-fast">v4-fast (faster)</option>
          <option value="v4-accurate">v4-accurate (better quality)</option>
        </select>
      </div>
      <div class="setting-row">
        <label>Max Clip Length</label>
        <input
          type="number"
          bind:value={maxClipLength}
          min="30"
          max="600"
          disabled={processing}
        />
      </div>
      <div class="setting-row">
        <label>Encoder</label>
        <select bind:value={encoder} disabled={processing}>
          <option value="hevc_nvenc">hevc_nvenc (GPU, fast)</option>
          <option value="h264_nvenc">h264_nvenc (GPU, fast)</option>
          <option value="libx265">libx265 (CPU, smaller file)</option>
          <option value="libx264">libx264 (CPU, compatible)</option>
        </select>
      </div>
      <div class="setting-row">
        <label>CRF / CQ (quality)</label>
        <input
          type="number"
          bind:value={crf}
          min="0"
          max="51"
          disabled={processing}
        />
        <span class="hint">Lower = better quality, bigger file</span>
      </div>
      <div class="setting-row">
        <label>Preset</label>
        <select bind:value={preset} disabled={processing}>
          <option value="ultrafast">ultrafast</option>
          <option value="fast">fast</option>
          <option value="medium">medium</option>
          <option value="slow">slow</option>
          <option value="veryslow">veryslow</option>
        </select>
      </div>
      <div class="setting-row">
        <label>Filename Prefix</label>
        <input type="text" bind:value={prefix} disabled={processing} />
      </div>
      <div class="setting-row">
        <label>
          <input
            type="checkbox"
            bind:checked={deleteOriginal}
            disabled={processing}
          />
          Delete original after success
        </label>
      </div>
      <div class="setting-row">
        <label>
          <input
            type="checkbox"
            bind:checked={sameDirectory}
            disabled={processing}
          />
          Output to same directory
        </label>
      </div>
      {#if !sameDirectory}
        <div class="setting-row">
          <label>Output Directory</label>
          <div class="dir-select">
            <input
              type="text"
              bind:value={outputDirectory}
              placeholder="Select output directory..."
              disabled={processing}
            />
            <button onclick={selectOutputDir} disabled={processing}>Browse</button>
          </div>
        </div>
      {/if}
    </div>
  {/if}

  <div
    class="file-list"
    class:dragging
    role="list"
  >
    {#if files.length === 0}
      <div class="empty-state">
        Drag & drop video files here, or click "Add Files"
      </div>
    {:else}
      {#each files as file, index}
        <div class="file-item" role="listitem">
          <div class="file-header">
            <span
              class="status-icon"
              style="color: {statusColor(file.status)}"
            >
              {statusIcon(file.status)}
            </span>
            <span class="file-name" title={file.path}>{file.name}</span>
            {#if file.status === "pending" && !processing}
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
              <span class="progress-text">
                {Math.round(file.progress)}%
                {#if file.status === "processing" && currentFileRemaining && currentFileRemaining !== "?"}
                  - {currentFileRemaining}
                {/if}
              </span>
            </div>
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
    {@const processingFile = files.find((f) => f.status === "processing")}
    {@const overallPct = totalFiles > 0 ? ((doneCount + (processingFile ? processingFile.progress / 100 : 0)) / totalFiles) * 100 : 0}
    <div class="overall-progress">
      <div class="overall-header">
        <span>Overall: {doneCount}/{totalFiles} files</span>
        <span class="overall-stats">
          {#if currentFileRemaining && currentFileRemaining !== "?"}
            Remaining: {currentFileRemaining}
          {/if}
          {#if currentFileSpeed && currentFileSpeed !== "?"}
            &nbsp;| Speed: {currentFileSpeed}
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
      <button class="cancel-btn" onclick={cancelProcessing}>Cancel</button>
    {:else}
      <button
        class="start-btn"
        onclick={startProcessing}
        disabled={files.length === 0 || !dockerOk}
      >
        Start Processing ({files.filter((f) => f.status === "pending").length} files)
      </button>
    {/if}
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

  h1 {
    margin: 0;
    font-size: 1.4em;
    color: #e94560;
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
    gap: 8px;
    flex-shrink: 0;
  }
  .spacer {
    flex: 1;
  }

  button {
    padding: 6px 14px;
    border: 1px solid #333;
    border-radius: 6px;
    background: #16213e;
    color: #eee;
    cursor: pointer;
    font-size: 0.85em;
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

  .settings-btn {
    background: #2d2d44;
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
  .hint {
    font-size: 0.8em;
    color: #666;
  }
  .dir-select {
    display: flex;
    gap: 6px;
    flex: 1;
  }
  .dir-select input {
    flex: 1;
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
  }
  .overall-bar {
    height: 22px;
  }

  footer {
    flex-shrink: 0;
    display: flex;
    justify-content: center;
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
    background: #ff9800;
    border-color: #ff9800;
    color: #fff;
    font-size: 1em;
    padding: 10px 40px;
    border-radius: 8px;
    font-weight: 600;
  }
  .cancel-btn:hover {
    background: #e68900;
  }
</style>
