# Lada GUI

**[한국어](README.ko.md)** | English | [![Discord](https://img.shields.io/badge/Discord-Join-7289da?logo=discord&logoColor=white)](https://discord.gg/px4vBjTUBg)

A lightweight batch front-end for AI video mosaic removal. It drives either
[Lada](https://github.com/ladaapp/lada) (via Docker) or [JASNA](https://github.com/Kruk2/jasna)
(native, faster) — the engine is a setting, and the app is built so that better engines can be
swapped in later.

Built with Tauri + Svelte (~3 MB installer). Queue many files, run them in parallel, tune settings
to your hardware automatically, and let it shut the PC down when done.

## Screenshots

| File Queue | Parallel Processing | Settings |
|:-:|:-:|:-:|
| ![GUI](.github/GUI.png) | ![Processing](.github/구동화면.png) | ![Settings](.github/세팅.png) |

## Features

- **Two engines, one queue** — Lada (Docker) or JASNA (native). Switch in settings; nothing else changes.
- **Auto settings** — detects GPU/VRAM, CPU cores, RAM, Docker's memory ceiling and free temp space,
  then sets parallel jobs, clip length, container memory, FP16 and encoder — and shows *why*.
- **Parallel processing** (up to 8 files at once). A single JASNA process does not saturate a big GPU,
  and JASNA's own GUI is sequential-only, so this is where the throughput comes from.
- **VR (side-by-side) videos** handled on both engines — see [VR videos](#vr-videos) below.
- **In-app updates** — a banner appears when a new release is out; one click installs it (signed).
- Real-time per-file progress with ETA and speed; drag & drop; delete original; shutdown after done.
- Retries are bounded and classified: a lost drive or missing file fails immediately, transient
  errors retry with backoff, and a job that gives up is marked failed rather than left "processing".
- Temp folders left by a crash or a dropped drive are swept automatically on the next job.

## Quick start

1. Install an NVIDIA driver (610+ recommended).
2. Pick an engine and set it up — **A** or **B** below. You can install both and switch any time.
3. Download `Lada GUI_x.x.x_x64-setup.exe` from [Releases](https://github.com/rhupy/ladaGUI/releases)
   and install.
4. In Settings, set **Settings Mode → Auto** (a fresh install starts in Auto already).
5. Drag videos in and press **Start Processing**.

### A. JASNA engine (recommended for speed and for VR)

The app **never downloads or bundles JASNA** — it only detects and runs an install you made yourself
(JASNA is AGPL, like Lada).

1. From the [JASNA releases](https://github.com/Kruk2/jasna/releases), download **every part** of the
   Windows (NVIDIA) package (`.7z.001`, `.002`, `.003`) and extract them together.
2. Install under an **ASCII-only path**, e.g. `C:\jasna` — that location is auto-detected. Anywhere
   else: enter the `jasna.exe` path in Settings → **JASNA Path**.
3. Requires an RTX 20-series or newer GPU and Windows driver 610+. The first run builds TensorRT
   engines once (a few minutes on an RTX 5090).
4. In Settings, set **Engine → JASNA**. The header should read `JASNA 0.10.0 ready`.

Docker is not needed on this engine. Pause is unavailable (a native CUDA process cannot be suspended
safely); Cancel works.

### B. Lada engine (Docker)

1. Install [Docker Desktop](https://www.docker.com/products/docker-desktop/) with the WSL 2 backend.
2. Enable GPU access: Settings → Resources → WSL Integration (enable your distro), and make sure the
   `nvidia` runtime is present under Settings → Docker Engine — otherwise install the
   [NVIDIA Container Toolkit](https://docs.nvidia.com/datacenter/cloud-native/container-toolkit/latest/install-guide.html).
3. Pull the image (~14 GB): `docker pull ladaapp/lada:latest`, or press **Update Lada** in the app.
4. Verify: `docker run --rm --gpus all ladaapp/lada:latest nvidia-smi` should print your GPU.
5. The header should read `Docker OK, GPU: NVIDIA`.

## VR videos

Side-by-side (SBS) VR is detected automatically — a 2:1 frame taller than 1080p whose left and right
halves are a stereo pair.

- **JASNA** processes the two eyes inside its own pipeline: one pass, one encode, no split/merge.
  The app detects VR (via the ffprobe JASNA ships) and automatically applies the combination that
  was confirmed by eye to remove the mosaic on real fisheye content — the VR-trained detector
  `rfdetr-vr-v1`, fisheye projection of mosaic regions, and a lower detection threshold. JASNA's
  own auto mode (generic detector, raw projection) left the mosaic untouched on the same clip.
- **Lada** cannot handle VR itself, so the app splits the file into left/right eyes, restores each
  at 4K, and rejoins them. That is three encode generations and, because a 4K eye needs a short clip
  window (20 frames) to fit in memory, noticeably less temporal stability. It works, but it is slow:
  a 60 s 8K clip took 699 s on Lada versus 107 s on JASNA on the same machine.

## Measured performance

Same files, same machine (RTX 5090, Ryzen 9 9950X3D):

| | Lada | JASNA |
|---|---|---|
| 90 s 1080p clip | 92 s | **35 s** (2.6×) |
| 3 files at once (JASNA) | — | **2.06×** the throughput of one at a time |
| 60 s 8K SBS VR | 699 s | **107 s** |

Clip length does not change Lada's speed (measured flat from 45 to 300 frames while VRAM doubled), so
Auto keeps it at 180 for temporal stability rather than pushing it higher.

## Settings

Auto mode manages the starred rows and locks them; switch to Manual to set them yourself.

| Setting | Default | Notes |
|---|---|---|
| Engine | Lada | `Lada` (Docker) or `JASNA` (native) |
| JASNA Path | *(auto)* | Only needed if `jasna.exe` is not in a well-known location |
| Settings Mode | Manual (Auto on fresh installs) | Auto derives the starred rows from detected hardware |
| Detection Model | v4-accurate / rfdetr-v6 | Lists differ per engine. VR files on JASNA switch to `rfdetr-vr-v1` automatically |
| Restoration Model | basicvsrpp-v1.2 | Lada only |
| FP16 ★ | auto | Half precision; on for Volta and newer |
| Max Clip Length ★ | 180 | **Frames**, not seconds. Higher = more temporal stability and more VRAM; past ~180 it stops paying off |
| Parallel Jobs ★ | from hardware | Bounded by VRAM, CPU cores and Docker's memory ceiling |
| Memory Limit ★ | from hardware | Per container, Lada only |
| Encoder ★ | hevc_nvenc | `h264_nvenc` / `libx265` / `libx264` |
| CRF / CQ | 18 | Lower = better quality, larger file |
| Preset | medium | Encoder speed/quality trade-off |
| Filename Prefix | [nm] | Prepended to output names |
| Output to same directory | On | Or choose an output folder |
| Delete original | On | After a successful run |
| Shutdown after | Off | Power off when the queue finishes |

## Updates

From v0.7.0 the app checks GitHub on startup. When a newer release exists a banner offers a one-click
update; it downloads, verifies the signature, installs and restarts. Updating is blocked while jobs
are running, because the Windows installer has to close the app.

## Logs

`lada-gui.log` next to the executable records every spawned command, retries, failures, hardware
detection and temp-folder sweeps.

## Troubleshooting

**`JASNA not found`** — make sure every archive part was extracted, the path has only ASCII characters,
and `jasna.exe --version` runs from a terminal. Then set the path in Settings if it is not `C:\jasna`.

**Mosaic still visible on a VR file (JASNA)** — check `lada-gui.log` for a `JASNA-PROBE ... VR(SBS)` line.
If the file was probed as 2D, its frame is not 2:1 or not taller than 1080p, which is the rule JASNA
itself uses for side-by-side VR.

**`Docker OK, GPU: not detected`** — Docker Desktop is running without GPU access; re-check step B.2
and restart Docker Desktop.

**`exit code 125` / "drive unavailable"** — the drive holding the video went away mid-job (common with
external disks). Reconnect it, then restart Docker Desktop: it keeps the broken mount until restarted.
The job fails immediately instead of retrying forever, so just re-queue it.

**`exit code 137`** — out of memory in the container. Raise Memory Limit or lower Max Clip Length; the
job retries on its own since a sibling job finishing often frees the memory.

**Pause is greyed out** — you are on the JASNA engine; use Cancel.

## Tech stack

- Frontend: Svelte 5 · Backend: Rust (Tauri v2)
- Engines: [Lada](https://github.com/ladaapp/lada) via Docker, [JASNA](https://github.com/Kruk2/jasna) native
- Updater: tauri-plugin-updater, signed releases

## Development

```bash
cd src
npm install
npm run tauri dev      # run
npm run tauri build    # package
cd src-tauri && cargo test   # backend tests
```

## License

MIT. Lada and JASNA are separate AGPL-3.0 projects; this app invokes them and bundles neither.
