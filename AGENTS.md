# AGENTS.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Development Commands

**Prerequisites:**

- [Rust](https://rustup.rs/) (latest stable)
- [Bun](https://bun.sh/) package manager

**Core Development:**

```bash
# Install dependencies
bun install

# Run in development mode
bun run tauri dev
# If cmake error on macOS:
CMAKE_POLICY_VERSION_MINIMUM=3.5 bun run tauri dev

# Build for production
bun run tauri build

# Frontend only development
bun run dev        # Start Vite dev server
bun run build      # Build frontend (TypeScript + Vite)
bun run preview    # Preview built frontend
```

**Model Setup (Required for Development):**

```bash
# Create models directory
mkdir -p src-tauri/resources/models

# Download required VAD model
curl -o src-tauri/resources/models/silero_vad_v4.onnx https://blob.handy.computer/silero_vad_v4.onnx
```

## Architecture Overview

Handy is a cross-platform desktop speech-to-text application built with Tauri (Rust backend + React/TypeScript frontend).

### Core Components

**Backend (Rust - src-tauri/src/):**

- `lib.rs` - Main application entry point with Tauri setup, tray menu, and managers
- `managers/` - Core business logic managers:
  - `audio.rs` - Audio recording and device management
  - `model.rs` - Whisper model downloading and management
  - `transcription.rs` - Speech-to-text processing pipeline
- `audio_toolkit/` - Low-level audio processing:
  - `audio/` - Device enumeration, recording, resampling
  - `vad/` - Voice Activity Detection using Silero VAD
- `commands/` - Tauri command handlers for frontend communication
- `shortcut.rs` - Global keyboard shortcut handling
- `settings.rs` - Application settings management

**Frontend (React/TypeScript - src/):**

- `App.tsx` - Main application component with onboarding flow
- `components/settings/` - Settings UI components
- `components/model-selector/` - Model management interface
- `hooks/` - React hooks for settings and model management
- `lib/types.ts` - Shared TypeScript type definitions

### Key Architecture Patterns

**Manager Pattern:** Core functionality is organized into managers (Audio, Model, Transcription) that are initialized at startup and managed by Tauri's state system.

**Command-Event Architecture:** Frontend communicates with backend via Tauri commands, backend sends updates via events.

**Pipeline Processing:** Audio → VAD → Whisper → Text output with configurable components at each stage.

### Technology Stack

**Core Libraries:**

- `whisper-rs` - Local Whisper inference with GPU acceleration
- `cpal` - Cross-platform audio I/O
- `vad-rs` - Voice Activity Detection
- `rdev` - Global keyboard shortcuts
- `rubato` - Audio resampling
- `rodio` - Audio playback for feedback sounds

**Platform-Specific Features:**

- macOS: Metal acceleration for Whisper, accessibility permissions
- Windows: Vulkan acceleration, code signing
- Linux: OpenBLAS + Vulkan acceleration

### Application Flow

1. **Initialization:** App starts minimized to tray, loads settings, initializes managers
2. **Model Setup:** First-run downloads preferred Whisper model (Small/Medium/Turbo/Large)
3. **Recording:** Global shortcut triggers audio recording with VAD filtering
4. **Processing:** Audio sent to Whisper model for transcription
5. **Output:** Text pasted to active application via system clipboard

### Settings System

Settings are stored using Tauri's store plugin with reactive updates:

- Keyboard shortcuts (configurable, supports push-to-talk)
- Audio devices (microphone/output selection)
- Model preferences (Small/Medium/Turbo/Large Whisper variants)
- Audio feedback and translation options

### Single Instance Architecture

The app enforces single instance behavior - launching when already running brings the settings window to front rather than creating a new process.

## Cursor Cloud specific instructions

### Environment

- Rust toolchain must be **stable** (set via `rustup default stable`). The VM ships with an older pinned toolchain that lacks `edition2024` support required by dependencies.
- The C/C++ compiler must be **gcc/g++** (not clang). Set `CC=gcc CXX=g++` before running `cargo build` or `bun run tauri dev`. Without this, whisper-rs-sys fails to find C++ standard library headers.
- `WEBKIT_DISABLE_DMABUF_RENDERER=1` should be set when launching the Tauri app to avoid WebKit rendering issues in the VM.
- `DISPLAY=:1` must be set for the Tauri desktop window to appear.

### Running the app

```bash
export CC=gcc CXX=g++ DISPLAY=:1 WEBKIT_DISABLE_DMABUF_RENDERER=1
bun run tauri dev
```

The first `tauri dev` run compiles the full Rust backend (~3-5 min). Subsequent runs with hot-reload are faster.

### Linting and checks

- `bun run lint` — ESLint on frontend
- `bun run format:check` — Prettier + cargo fmt check
- `cargo clippy` (in `src-tauri/`) — Rust linting (expect ~30 pre-existing warnings)

### Key gotchas

- The frontend alone (`bun run dev`) shows a blank page in the browser because it depends on Tauri APIs. Always use `bun run tauri dev` for full testing.
- The Silero VAD model file at `src-tauri/resources/models/silero_vad_v4.onnx` is required at runtime. If missing, download it (see Model Setup in the Development Commands section above).
- No external services (databases, APIs, Docker) are needed — everything runs locally.
