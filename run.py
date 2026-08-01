#!/usr/bin/env python3
"""Run the full Vorexis stack (engine + backend + frontend) with one command."""
import argparse
import os
import re
import signal
import socket
import subprocess
import sys
import threading
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent
BACKEND = ROOT / "backend"
FRONTEND = ROOT / "frontend"
ENGINE = ROOT / "engine"
VENV = BACKEND / ".venv"
ENGINE_BIN = ENGINE / "target" / "release" / "aipentest-engine"

BACKEND_PORT = 8000
FRONTEND_PORT = 5173

CHILDREN: list[subprocess.Popen] = []
LOCK = threading.Lock()
STOPPING = threading.Event()


def log(tag: str, msg: str) -> None:
    print(f"\033[2m[{tag}]\033[0m {msg}", flush=True)


def port_pid(port: int) -> list[int]:
    out = subprocess.run(
        ["ss", "-ltnp", f"sport = :{port}"], capture_output=True, text=True
    ).stdout
    pids = set()
    for m in re.finditer(r"pid=(\d+)", out):
        pids.add(int(m.group(1)))
    return sorted(pids)


def port_open(port: int) -> bool:
    try:
        with socket.create_connection(("127.0.0.1", port), timeout=0.5):
            return True
    except OSError:
        return False


def check_ports(kill: bool) -> None:
    for port in (BACKEND_PORT, FRONTEND_PORT):
        if port_open(port):
            pids = port_pid(port)
            if kill:
                for pid in pids:
                    log("run", f"killing process {pid} on port {port}")
                    try:
                        os.kill(pid, signal.SIGTERM)
                    except ProcessLookupError:
                        pass
                time.sleep(1.5)
                if port_open(port):
                    for pid in pids:
                        try:
                            os.kill(pid, signal.SIGKILL)
                        except ProcessLookupError:
                            pass
                    time.sleep(0.5)
            else:
                print(
                    f"\033[31m[run] port {port} sudah dipakai "
                    f"(pid {', '.join(map(str, pids))}) — jalankan dengan --kill\033[0m"
                )
                sys.exit(1)


def ensure_venv(no_setup: bool) -> None:
    python = VENV / "bin" / "python"
    if not python.exists():
        if no_setup:
            print("[run] backend/.venv tidak ada — jalankan tanpa --no-setup dulu")
            sys.exit(1)
        log("setup", "membuat venv + install requirements (sekali saja)...")
        subprocess.run([sys.executable, "-m", "venv", str(VENV)], check=True)
        subprocess.run(
            [str(python), "-m", "pip", "install", "-q", "-r", str(BACKEND / "requirements.txt")],
            check=True,
        )
    if no_setup:
        return
    req = (BACKEND / "requirements.txt").read_text()
    if any(p not in req for p in ("fastapi", "uvicorn")):
        return


def ensure_frontend_deps(no_setup: bool) -> None:
    if (FRONTEND / "node_modules").exists():
        return
    if no_setup:
        print("[run] frontend/node_modules tidak ada — jalankan tanpa --no-setup dulu")
        sys.exit(1)
    log("setup", "npm install (sekali saja)...")
    subprocess.run(["npm", "install", "--no-audit", "--no-fund"], cwd=FRONTEND, check=True)


def build_engine(force: bool) -> None:
    if ENGINE_BIN.exists() and not force:
        log("engine", "binary sudah ada, skip build (pakai --force-engine-build untuk rebuild)")
        return
    if not shutil_which("cargo"):
        print("[run] cargo tidak ditemukan — install Rust dulu: https://rustup.rs")
        sys.exit(1)
    log("engine", "cargo build --release...")
    run_child(["cargo", "build", "--release"], cwd=ENGINE, tag="engine", wait=True)
    if not ENGINE_BIN.exists():
        print("[run] build engine gagal")
        sys.exit(1)


def shutil_which(name: str) -> bool:
    for path in os.environ.get("PATH", "").split(":"):
        if os.path.isfile(os.path.join(path, name)):
            return True
    return False


def pump(stream, tag: str) -> None:
    for line in iter(stream.readline, ""):
        line = line.rstrip("\n")
        if line:
            log(tag, line)
    stream.close()


def run_child(cmd: list[str], cwd: Path, tag: str, wait: bool = False) -> subprocess.Popen | None:
    p = subprocess.Popen(
        cmd,
        cwd=cwd,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        bufsize=1,
        start_new_session=True,
    )
    with LOCK:
        CHILDREN.append(p)
    threading.Thread(target=pump, args=(p.stdout, tag), daemon=True).start()
    if wait:
        p.wait()
        with LOCK:
            CHILDREN.remove(p)
        return None
    return p


def shutdown(signum=None, frame=None) -> None:
    if STOPPING.is_set():
        return
    STOPPING.set()
    print("\n\033[33m[run] menghentikan semua proses...\033[0m", flush=True)
    with LOCK:
        procs = list(CHILDREN)
    for p in procs:
        try:
            os.killpg(p.pid, signal.SIGTERM)
        except (OSError, ProcessLookupError):
            pass
    for p in procs:
        try:
            p.wait(timeout=8)
        except subprocess.TimeoutExpired:
            try:
                os.killpg(p.pid, signal.SIGKILL)
            except (OSError, ProcessLookupError):
                pass
            try:
                p.wait(timeout=3)
            except subprocess.TimeoutExpired:
                pass
    sys.exit(0)


def main() -> None:
    ap = argparse.ArgumentParser(description="Jalankan Vorexis: engine + backend + frontend")
    ap.add_argument("--kill", action="store_true", help="matikan proses lama yang memakai port 8000/5173")
    ap.add_argument("--prod", action="store_true", help="frontend mode produksi (build + serve)")
    ap.add_argument("--force-engine-build", action="store_true", help="rebuild engine Rust")
    ap.add_argument("--no-setup", action="store_true", help="skip setup otomatis (venv/npm install)")
    args = ap.parse_args()

    check_ports(args.kill)
    ensure_venv(args.no_setup)
    ensure_frontend_deps(args.no_setup)
    build_engine(args.force_engine_build)

    uvicorn = VENV / "bin" / "uvicorn"
    run_child(
        [str(uvicorn), "app.main:app", "--port", str(BACKEND_PORT), "--app-dir", str(BACKEND)],
        cwd=BACKEND,
        tag="backend",
    )
    log("backend", f"http://localhost:{BACKEND_PORT}")

    if args.prod:
        run_child(["npm", "run", "build"], cwd=FRONTEND, tag="frontend", wait=True)
        run_child(["node", "build"], cwd=FRONTEND, tag="frontend", wait=False)
        log("frontend", f"http://localhost:{FRONTEND_PORT} (production)")
    else:
        run_child(["npm", "run", "dev", "--", "--port", str(FRONTEND_PORT)], cwd=FRONTEND, tag="frontend")
        log("frontend", f"http://localhost:{FRONTEND_PORT}")

    print("\n\033[32m[run] semua service berjalan. Ctrl+C untuk berhenti.\033[0m", flush=True)

    signal.signal(signal.SIGINT, shutdown)
    signal.signal(signal.SIGTERM, shutdown)
    try:
        while not STOPPING.is_set():
            alive = [p.poll() for p in list(CHILDREN) if p.poll() is None]
            if not alive:
                print("\033[31m[run] tidak ada proses yang berjalan — keluar\033[0m")
                shutdown()
            time.sleep(1)
    except KeyboardInterrupt:
        shutdown()


if __name__ == "__main__":
    main()
