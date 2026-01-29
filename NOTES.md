# HEXWAR Balancer - Setup Notes

## Install Notes

### Dependencies
- Python 3.13+
- Rust (installed via rustup)
- maturin (for building Rust extension)
- numpy, pytest

### Setup Steps

1. Create/activate virtual environment:
   ```bash
   cd /home/giblfiz/hexwar
   python3 -m venv .venv
   source .venv/bin/activate
   ```

2. Install Python dependencies:
   ```bash
   pip install numpy maturin pytest
   ```

3. Build Rust extension (required for web server):
   ```bash
   source "$HOME/.cargo/env"  # If Rust newly installed
   cd hexwar_core && maturin develop --release
   ```

4. Start the web server:
   ```bash
   source .venv/bin/activate
   python3 -m hexwar.visualizer.server
   ```

### Running the Server

```bash
cd /home/giblfiz/hexwar
source .venv/bin/activate
nohup python3 -m hexwar.visualizer.server > /tmp/hexwar-server.log 2>&1 &
```

Check server status:
```bash
curl -s http://localhost:8002/api/board | head -1
```

## Process Notes

### Initial Setup (Jan 29, 2026)
- Project uploaded via file-drop as `hexwar-balancer.zip`
- Original venv was for ARM64 Mac (Python 3.12), recreated for x86_64 Linux (Python 3.13)
- Installed Rust via rustup for building hexwar_core extension
- Built Rust extension with maturin in release mode

### Integration Issues
- The .venv from Mac had incompatible architecture binaries
- Rust extension (hexwar_core) is required for the visualizer server - no fallback
- Server runs on port 8002 by default (hardcoded in server.py)

## Web Interfaces

The server provides three HTML interfaces:

| Page | URL | Purpose |
|------|-----|---------|
| designer.html | /designer.html | Board designer - edit army compositions |
| index.html | /index.html | Game viewer - playback recorded games |
| player.html | /player.html | Interactive player - play against AI |

Root `/` redirects to `/designer.html`.

## Port & Subdomain

- **Port:** 8002
- **Subdomain:** hexwar.implausible.enterprises
- **Nginx:** Added to `/etc/nginx/sites-available/implausible-enterprises`
