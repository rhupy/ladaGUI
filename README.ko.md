# Lada GUI

한국어 | **[English](README.md)** | [![Discord](https://img.shields.io/badge/Discord-Join-7289da?logo=discord&logoColor=white)](https://discord.gg/px4vBjTUBg)

AI 영상 모자이크 제거를 위한 가벼운 배치 프론트엔드입니다. [Lada](https://github.com/ladaapp/lada)(Docker) 또는
[JASNA](https://github.com/Kruk2/jasna)(네이티브, 더 빠름)를 엔진으로 구동하며 — 엔진은 설정값일 뿐이라
더 좋은 엔진이 나오면 갈아끼울 수 있는 구조입니다.

Tauri + Svelte (~3MB 설치파일). 파일을 여러 개 큐에 넣고 병렬로 돌리고, 사양에 맞춰 설정을 자동으로 맞추고,
끝나면 PC를 끄게 할 수 있습니다.

## Screenshots

| 파일 큐 | 병렬 처리 | 설정 |
|:-:|:-:|:-:|
| ![GUI](.github/GUI.png) | ![Processing](.github/구동화면.png) | ![Settings](.github/세팅.png) |

## 기능

- **엔진 2종, 큐는 하나** — Lada(Docker) 또는 JASNA(네이티브). 설정에서 바꾸면 그 외엔 그대로입니다.
- **자동 설정** — GPU/VRAM, CPU 코어, RAM, Docker 메모리 상한, 임시폴더 여유를 감지해 병렬 수·클립 길이·
  컨테이너 메모리·FP16·인코더를 정하고, **왜 그렇게 정했는지**도 보여줍니다.
- **병렬 처리** (최대 8개 동시). JASNA 하나로는 큰 GPU를 다 못 쓰고 JASNA 자체 GUI는 순차 전용이라,
  처리량 차이는 여기서 납니다.
- **VR(좌우 분할) 영상** 양쪽 엔진 모두 지원 — 아래 [VR 영상](#vr-영상) 참고.
- **앱 내 자동 업데이트** — 새 릴리스가 나오면 배너가 뜨고 클릭 한 번에 설치됩니다(서명 검증).
- 파일별 실시간 진행률·남은시간·속도, 드래그&드롭, 원본 삭제, 완료 후 종료.
- 재시도는 상한이 있고 원인별로 나뉩니다: 드라이브 유실·파일 없음은 즉시 실패, 일시적 오류는 간격을 늘려
  재시도, 포기한 작업은 "처리 중"에 남지 않고 실패로 표시됩니다.
- 크래시나 드라이브 끊김으로 남은 임시 폴더는 다음 작업 때 자동으로 정리됩니다.

## 빠른 시작

1. NVIDIA 드라이버 설치 (610 이상 권장).
2. 엔진을 고르고 준비 — 아래 **A** 또는 **B**. 둘 다 설치해두고 언제든 바꿔도 됩니다.
3. [Releases](https://github.com/rhupy/ladaGUI/releases)에서 `Lada GUI_x.x.x_x64-setup.exe`를 받아 설치.
4. 설정에서 **설정 방식 → 자동** (신규 설치는 처음부터 자동입니다).
5. 영상을 끌어다 놓고 **처리 시작**.

### A. JASNA 엔진 (속도와 VR에 권장)

이 앱은 JASNA를 **내려받거나 번들하지 않습니다** — 직접 설치한 것을 찾아 실행만 합니다 (JASNA는 Lada와
같은 AGPL).

1. [JASNA releases](https://github.com/Kruk2/jasna/releases)에서 Windows(NVIDIA) 패키지의 **모든 파트**
   (`.7z.001`, `.002`, `.003`)를 받아 함께 압축 해제.
2. **영문·숫자만 있는 경로**에 설치 — 예: `C:\jasna` (자동 탐색 위치). 다른 곳이면 설정 → **JASNA 경로**에
   `jasna.exe` 위치를 입력.
3. RTX 20 시리즈 이상, Windows 드라이버 610 이상 필요. 첫 실행 시 TensorRT 엔진을 한 번 빌드합니다
   (RTX 5090 기준 몇 분).
4. 설정에서 **엔진 → JASNA**. 상단이 `JASNA 0.10.0 ready`로 바뀌면 준비된 것입니다.

이 엔진에선 Docker가 필요 없습니다. 일시정지는 지원하지 않습니다(네이티브 CUDA 프로세스를 안전하게
멈출 수 없음). 취소는 정상 동작합니다.

### B. Lada 엔진 (Docker)

1. [Docker Desktop](https://www.docker.com/products/docker-desktop/)을 WSL 2 백엔드로 설치.
2. GPU 접근 설정: Settings → Resources → WSL Integration에서 배포판 활성화, Settings → Docker Engine에
   `nvidia` 런타임이 있는지 확인 — 없으면
   [NVIDIA Container Toolkit](https://docs.nvidia.com/datacenter/cloud-native/container-toolkit/latest/install-guide.html) 설치.
3. 이미지 받기(~14GB): `docker pull ladaapp/lada:latest`, 또는 앱의 **Lada 업데이트** 버튼.
4. 확인: `docker run --rm --gpus all ladaapp/lada:latest nvidia-smi` 에 GPU가 보이면 됩니다.
5. 상단이 `Docker OK, GPU: NVIDIA`로 표시됩니다.

## VR 영상

좌우 분할(SBS) VR은 자동으로 감지합니다 — 1080p보다 크고 2:1 비율이며 좌/우 절반이 스테레오 쌍인 영상.

- **JASNA**는 두 눈을 자기 파이프라인 안에서 처리합니다: 1패스, 인코딩 1회, 분리·합성 없음.
  앱이 VR을 감지하면(JASNA 번들 ffprobe 사용) 실제 어안 콘텐츠에서 **육안으로 모자이크 제거가 확인된
  조합**을 자동 적용합니다 — 모자이크 영역 어안 투영 + VR 학습 탐지기 `rfdetr-vr-v1`. JASNA 자체 자동 모드는 미등록
  스튜디오를 raw 투영으로 처리하는데, raw 프레임에선 같은 클립의 모자이크가 탐지되지 않았습니다.
- **Lada**는 VR을 직접 처리하지 못해, 앱이 좌/우 눈으로 분리해 각각 4K로 복원한 뒤 다시 합칩니다.
  인코딩이 3세대를 거치고, 4K 한쪽 눈이 메모리에 들어가려면 클립을 20프레임으로 줄여야 해서 시간적
  안정성도 떨어집니다. 동작은 하지만 느립니다: 같은 PC에서 8K 60초 클립이 Lada 699초, JASNA 107초.

## 실측 성능

같은 파일, 같은 PC (RTX 5090, Ryzen 9 9950X3D):

| | Lada | JASNA |
|---|---|---|
| 1080p 90초 클립 | 92초 | **35초** (2.6배) |
| 3개 동시 (JASNA) | — | 1개씩 대비 처리량 **2.06배** |
| 8K SBS VR 60초 | 699초 | **107초** |

클립 길이는 Lada 속도를 바꾸지 않습니다(45~300프레임에서 시간은 평평, VRAM은 2배로 측정). 그래서 자동
모드는 속도가 아닌 시간적 안정성을 위해 180에 둡니다.

## 설정

자동 모드는 ★ 표시된 항목을 직접 정하고 잠급니다. 직접 정하려면 수동으로 바꾸세요.

| 설정 | 기본값 | 비고 |
|---|---|---|
| 엔진 | Lada | `Lada`(Docker) 또는 `JASNA`(네이티브) |
| JASNA 경로 | *(자동)* | `jasna.exe`가 자동 탐색 위치에 없을 때만 |
| 설정 방식 | 수동 (신규 설치는 자동) | 자동은 감지된 사양으로 ★ 항목을 정합니다 |
| 감지 모델 | v4-accurate / rfdetr-v6 | 엔진별로 목록이 다릅니다. JASNA의 VR 파일은 자동으로 `rfdetr-vr-v1`을 씁니다 |
| 복원 모델 | basicvsrpp-v1.2 | Lada 전용 |
| FP16 ★ | 자동 | 반정밀도. Volta 이상은 켜짐 |
| 최대 클립 길이 ★ | 180 | **프레임** 단위(초 아님). 클수록 안정성↑ VRAM↑, ~180 이상은 이득이 없음 |
| 병렬 작업 수 ★ | 사양 기반 | VRAM·CPU 코어·Docker 메모리 상한으로 제한 |
| 컨테이너 메모리 ★ | 사양 기반 | Lada 전용 |
| 인코더 ★ | hevc_nvenc | `h264_nvenc` / `libx265` / `libx264` |
| CRF / CQ | 18 | 낮을수록 고화질·큰 파일 |
| 프리셋 | medium | 인코딩 속도/품질 |
| 파일명 접두어 | [nm] | 출력 파일명 앞에 붙음 |
| 같은 디렉토리에 출력 | 켜짐 | 또는 출력 폴더 지정 |
| 성공 후 원본 삭제 | 켜짐 | 성공한 경우에만 |
| 완료 후 PC 종료 | 꺼짐 | 큐가 끝나면 전원 종료 |

## 업데이트

v0.7.0부터 시작 시 GitHub를 확인합니다. 새 릴리스가 있으면 배너가 뜨고 클릭 한 번에 내려받아 서명을
검증한 뒤 설치·재시작합니다. Windows 설치 과정에서 앱이 종료되므로 작업 중에는 업데이트가 막힙니다.

## 로그

실행파일 옆의 `lada-gui.log`에 실행된 명령, 재시도, 실패, 하드웨어 감지, 임시 폴더 정리가 기록됩니다.

## 문제 해결

**`JASNA를 찾지 못했습니다`** — 압축 파트가 전부 풀렸는지, 경로에 영문·숫자만 있는지, 터미널에서
`jasna.exe --version`이 되는지 확인하세요. `C:\jasna`가 아니면 설정에 경로를 입력하세요.

**VR 파일에 모자이크가 그대로 남음 (JASNA)** — `lada-gui.log`에서 `JASNA-PROBE ... VR(SBS)` 줄을 확인하세요.
2D로 판별됐다면 프레임이 2:1이 아니거나 1080p보다 크지 않은 것으로, JASNA 자신이 쓰는 SBS 판별 규칙입니다.

**`Docker OK, GPU: not detected`** — Docker Desktop이 GPU 접근 없이 실행 중입니다. B.2 단계를 다시 확인하고
Docker Desktop을 재시작하세요.

**`exit code 125` / "drive unavailable"** — 작업 중 영상이 있는 드라이브가 사라졌습니다(외장하드에서
흔함). 다시 연결한 뒤 **Docker Desktop을 재시작**하세요 — 재시작 전까지 깨진 마운트를 물고 있습니다.
작업은 무한 재시도 대신 즉시 실패하니 다시 큐에 넣으면 됩니다.

**`exit code 137`** — 컨테이너 메모리 부족. 컨테이너 메모리를 올리거나 최대 클립 길이를 낮추세요.
옆 작업이 끝나면 메모리가 풀리는 경우가 많아 자동으로 재시도합니다.

**일시정지가 비활성** — JASNA 엔진입니다. 취소를 쓰세요.

## Tech Stack

- 프론트엔드: Svelte 5 · 백엔드: Rust (Tauri v2)
- 엔진: [Lada](https://github.com/ladaapp/lada) (Docker), [JASNA](https://github.com/Kruk2/jasna) (네이티브)
- 업데이터: tauri-plugin-updater, 서명된 릴리스

## Development

```bash
cd src
npm install
npm run tauri dev      # 실행
npm run tauri build    # 패키징
cd src-tauri && cargo test   # 백엔드 테스트
```

## License

MIT. Lada와 JASNA는 별개의 AGPL-3.0 프로젝트이며, 이 앱은 둘을 호출만 하고 번들하지 않습니다.
