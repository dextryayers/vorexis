# Vorexis — AI-Assisted Web Application Security Scanner (AIPentest)

Full-stack web application security scanning tool with a Rust scanning engine,
a FastAPI backend, and a SvelteKit frontend. Scans run through the local Rust
engine; results are analyzed and summarized by an AI assistant with a
provider fallback chain.

## Architecture

| Component | Directory  | Tech                                        |
| --------- | ---------- | ------------------------------------------- |
| Engine    | `engine/`  | Rust, async (tokio + reqwest), parallel probes |
| Backend   | `backend/` | FastAPI, SQLite (WAL), JWT auth, SSE streaming, WebSocket scan progress |
| Frontend  | `frontend/`| SvelteKit (Svelte 5 runes), Tailwind CSS v4  |

## Features

- 14 scan modules: DNS, TLS/SSL, HTTP(s) analysis, headers & cookies, methods,
  tech detection, WAF detection, fingerprinting, crawler, directory scanner
  (soft-404 aware), fuzzer (path traversal payloads), subdomain enumeration
  (wildcard DNS aware), port scanner, HTML parser (CSRF / insecure action)
- AI assistant chat with streaming (SSE), scan context attachment, and
  AI-generated scan reports
- AI provider fallback chain: OpenAI → Gemini → Ollama (local) → HuggingFace
- Live scan progress via WebSocket (per-module events, stop/cancel, recovery)
- Auth: JWT + rate limiting; SQLite tuned (WAL, busy timeout, indexes)

## Prerequisites

- Rust (edition 2024)
- Python 3.11+
- Node.js 18+
- (Optional) Ollama with a model (e.g. `gemma3:4b`) for local AI, or API keys
  for OpenAI/Gemini/HuggingFace

## Quick Start

Jalankan semuanya (engine build otomatis + backend + frontend) dari direktori root:

```bash
python3 run.py
```

Setup otomatis: membuat `backend/.venv` + install requirements, `npm install`, dan
`cargo build --release` jika belum ada. Opsi: `--kill` (matikan proses lama di port
8000/5173), `--prod` (frontend production build), `--force-engine-build` (rebuild
engine), `--no-setup` (skip setup otomatis).

Atau jalankan manual satu per satu:

### 1. Engine

```bash
cd engine
cargo build --release
```

### 2. Backend

```bash
cd backend
python -m venv .venv
.venv/bin/pip install -r requirements.txt
cp .env.example .env   # optionally set AI keys
.venv/bin/uvicorn app.main:app --port 8000
```

### 3. Frontend

```bash
cd frontend
npm install
npm run dev -- --port 5173
```

Open http://localhost:5173, register a user, and start scanning.

## Sessions & routes

Setiap chat & scan punya route sendiri (mirip session ChatGPT):

| Route          | Konten                                  |
| -------------- | --------------------------------------- |
| `/`            | Halaman baru (welcome + composer)       |
| `/c/<chat-id>` | Session chat (id random 32-hex) + status scan + link hasil |
| `/scan/<id>`   | Hasil scan lengkap (tabs per modul, AI report) |

Sidebar selalu tampil: pilih chat/scan lama, hapus chat, atau mulai baru. Shortcut `/` fokus composer.

## Configuration

All settings are read from the environment or a `.env` file inside `backend/`
(see `backend/.env.example`). Important ones:

| Variable                  | Default                     | Description                              |
| ------------------------- | --------------------------- | ---------------------------------------- |
| `SECRET_KEY`              | dev placeholder (rotate!)   | JWT signing secret                       |
| `DATABASE_URL`            | `backend/data/aipentest.db` | SQLite database path                     |
| `ENGINE_BINARY`           | `engine/target/release/...` | Rust engine binary path                  |
| `DEFAULT_AI_PROVIDER`     | `openai`                    | Ordered fallback: requested → default → all available |
| `OPENAI_API_KEY`          | —                           | OpenAI key                               |
| `GEMINI_API_KEY`          | —                           | Google Gemini key                        |
| `OLLAMA_BASE_URL`         | `http://localhost:11434`    | Local Ollama endpoint (no key needed)    |
| `HF_API_KEY`              | —                           | HuggingFace key                          |

## Development

```bash
# Engine build & checks
cd engine && cargo build --release

# Backend lint/type hints
cd backend && .venv/bin/python -m compileall app

# Frontend type check
cd frontend && npm run check

# Production build
cd frontend && npm run build
```

## License

All rights reserved.
