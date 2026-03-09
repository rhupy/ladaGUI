# Lada GUI

**[한국어](README.ko.md)** | English | [![Discord](https://img.shields.io/badge/Discord-Join-7289da?logo=discord&logoColor=white)](https://discord.gg/px4vBjTUBg)

A lightweight GUI wrapper for [Lada](https://github.com/ladaapp/lada), an AI-based video mosaic removal tool.

Built with Tauri + Svelte (~2.6MB installer). Runs Lada via Docker and displays real-time progress.

## Screenshots

| File Queue | Parallel Processing | Settings |
|:-:|:-:|:-:|
| ![GUI](.github/GUI.png) | ![Processing](.github/구동화면.png) | ![Settings](.github/세팅.png) |

## Features

- Drag & drop or browse to add video files
- Real-time progress bar with ETA and speed
- GPU encoding (NVENC) - minimal CPU load
- Parallel processing (1~4 simultaneous files)
- Auto-retry on failure (infinite, with logging)
- Delete original after success
- Auto shutdown PC after completion
- Custom output path / filename prefix
- One-click Lada Docker image update

## Prerequisites

### 1. NVIDIA GPU Driver

An NVIDIA GPU is required. Install the latest driver.

- [NVIDIA Driver Download](https://www.nvidia.com/Download/index.aspx)
- Driver version 570.0+ recommended (NVENC support)

### 2. Docker Desktop

Lada runs inside a Docker container.

1. Download and install [Docker Desktop](https://www.docker.com/products/docker-desktop/)
2. Use **WSL 2 backend** during installation (default)
3. Launch Docker Desktop after installation

### 3. Docker Desktop GPU Setup

GPU access in Docker requires additional configuration.

1. Open Docker Desktop
2. Settings > Resources > WSL Integration - enable your WSL distro
3. Settings > Docker Engine - verify the following exists:
   ```json
   {
     "runtimes": {
       "nvidia": {
         "path": "nvidia-container-runtime",
         "runtimeArgs": []
       }
     }
   }
   ```
   If missing, install [NVIDIA Container Toolkit](https://docs.nvidia.com/datacenter/cloud-native/container-toolkit/latest/install-guide.html).

### 4. Pull Lada Docker Image

Pre-download the Lada image (~14GB):

```bash
docker pull ladaapp/lada:latest
```

Or use the **Update Lada** button in the app.

### 5. Verify GPU Access (Optional)

```bash
docker run --rm --gpus all ladaapp/lada:latest nvidia-smi
```

If GPU info is displayed, you're good to go.

## Installation

1. Download `Lada GUI_x.x.x_x64-setup.exe` from [Releases](https://github.com/rhupy/lada/releases)
2. Install and launch
3. Verify `Docker OK, GPU: NVIDIA` in the top-right corner
4. Drag video files or click Add Files
5. Click Start Processing

## Settings

| Setting | Default | Description |
|---------|---------|-------------|
| Detection Model | v4-accurate | `v4-fast`: faster, `v4-accurate`: better quality |
| Max Clip Length | 300 | Split long videos into N-second segments (reduce if VRAM is low) |
| Parallel Jobs | 1 | Simultaneous files (increase with VRAM headroom, 2~4) |
| Encoder | hevc_nvenc | GPU encoding. Use `libx265` if no GPU |
| CRF / CQ | 18 | Quality (lower = better quality, larger file) |
| Preset | medium | Encoding speed vs quality tradeoff |
| Filename Prefix | [nm] | Prefix added to output filename |
| Delete original | On | Delete source file after successful processing |
| Shutdown after | Off | Shutdown PC after all files complete |
| Output to same directory | On | Save output in same folder as source |

## Recommended Settings by GPU

### RTX 5090 / 4090 (VRAM 24~32GB)
- Parallel Jobs: 2~4
- Encoder: hevc_nvenc
- Max Clip Length: 300~600

### RTX 4070 / 3070 (VRAM 8~12GB)
- Parallel Jobs: 1~2
- Encoder: hevc_nvenc
- Max Clip Length: 120~300

### No GPU / Low-end
- Parallel Jobs: 1
- Encoder: libx265
- Max Clip Length: 60~120

## Logs

Failure/retry logs are saved to `lada-gui.log` in the app installation directory.

## Troubleshooting

### "Docker OK, GPU: not detected"
- Ensure Docker Desktop is running
- Verify NVIDIA driver is installed
- Restart Docker Desktop and try again

### Immediate failure on start (exit code 125)
- Check if GPU support is enabled in Docker Desktop
- Test with: `docker run --rm --gpus all ladaapp/lada:latest nvidia-smi`

### hevc_nvenc encoder error
- Switch Encoder to `libx265` as a fallback
- NVIDIA driver 570.0+ is required for NVENC

### Processing hangs
- The app waits indefinitely as long as the process is alive
- Try Cancel and restart
- Failed processes are automatically retried

## Tech Stack

- **Frontend**: Svelte 5
- **Backend**: Rust (Tauri v2)
- **AI Engine**: [Lada](https://github.com/ladaapp/lada) (Docker)
- **Build**: ~2.6MB Windows installer

## Development

```bash
cd src

# Install dependencies
npm install

# Run in development mode
npm run tauri dev

# Build for production
npm run tauri build
```

## License

MIT
