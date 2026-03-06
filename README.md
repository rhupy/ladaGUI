# Lada GUI

AI 기반 동영상 모자이크 제거 도구 [Lada](https://github.com/ladaapp/lada)의 GUI 래퍼 애플리케이션.

## Features

- 파일 드래그앤드롭 / 추가 버튼으로 작업 큐 관리
- 실시간 프로그레스바
- 출력 경로 지정 (동일 경로 출력 옵션)
- 파일명 접두사 설정 (기본: `[nm]`)
- Lada 설정 커스터마이징 (detection model, CRF, max-clip-length 등)
- Docker 기반 Lada 자동 업데이트

## Prerequisites

- [Docker Desktop](https://www.docker.com/products/docker-desktop/) (GPU 지원 필요)
- NVIDIA GPU + 드라이버

## Tech Stack

- **Frontend**: Svelte
- **Backend**: Rust (Tauri)
- **Mosaic Removal**: Lada (Docker)

## Development

```bash
# Install dependencies
npm install

# Run in development mode
npm run tauri dev

# Build for production
npm run tauri build
```

## License

MIT
