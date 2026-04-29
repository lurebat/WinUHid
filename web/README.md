# `winuhid-web`

A browser-based control panel for the WinUHid driver. Lets you create
virtual HID devices, drive them from the browser, and watch the host
OS talk back (rumble, LEDs, adaptive triggers, raw HID events).

* **Backend:** Rust (axum + tokio + libloading).
* **Frontend:** plain HTML/CSS/ES2020+, no build step. The frontend is
  baked into the binary by `rust-embed`, so the only file you ship is
  `winuhid-web.exe`.
* **OS support:** Windows only (the WinUHid driver is Windows-only).

## Quick start

```powershell
# from the repo root
just web-run                     # debug build, http://127.0.0.1:7878
just web-run-release             # release build
```

If you don't have `just`:

```powershell
$env:WINUHID_DLL_DIR = "build\Release\x64"
cargo run --manifest-path web/Cargo.toml -- --addr 127.0.0.1:7878
```

You'll need the WinUHid driver **installed** on the host
(see the top-level [`README.md`](../README.md#installing-the-driver))
and `WinUHid.dll` / `WinUHidDevs.dll` available somewhere the app can
find them.

## How DLLs are located

`WinUHid.dll` is required, `WinUHidDevs.dll` is optional (without it,
the four preset tabs are disabled). Search order is:

1. Each `--dll-dir` you pass on the command line.
2. The `WINUHID_DLL_DIR` environment variable.
3. The current working directory.
4. The directory containing the `winuhid-web.exe` binary.
5. Whatever `LoadLibraryA` finds via PATH / system32 / etc.

The first hit wins.

## Features

* **Mouse** — Microsoft Precision Mouse: motion, 5 buttons, scroll
  wheel (vertical + horizontal).
* **DualShock 4** — sticks (drag-to-aim), triggers, hat, face buttons,
  shoulders, sticks-as-buttons, share/options/home/touchpad. Live
  rumble + lightbar feedback in the UI.
* **DualSense** — DualShock 4 plus mute button, with adaptive trigger
  effects, lightbar, and player-LED indicators surfaced live.
* **Xbox One controller** — sticks (16-bit), 10-bit triggers, ABXY,
  bumpers, hat, Home, with rumble + impulse-trigger motor feedback.
* **Generic HID** — paste any raw report descriptor in hex; submit any
  input report in hex; live `WRITE_REPORT` / `SET_FEATURE` event log.

## REST + WebSocket schema

Every endpoint that mutates state takes/returns JSON.

| Method | Path | Description |
| --- | --- | --- |
| `GET`    | `/api/health`                          | Driver version + whether `WinUHidDevs.dll` loaded. |
| `GET`    | `/api/devices`                         | List of created devices. |
| `DELETE` | `/api/devices/:id`                     | Destroy a device. |
| `POST`   | `/api/devices/generic`                 | Create a generic HID device (body: `GenericDeviceParams`). |
| `POST`   | `/api/devices/:id/generic/input`       | Submit a raw input report (`{ "hex": "…" }`). |
| `POST`   | `/api/devices/mouse`                   | Create a mouse (`{ "name": "…" }`). |
| `POST`   | `/api/devices/:id/mouse/motion`        | `{ "dx": int, "dy": int }`. |
| `POST`   | `/api/devices/:id/mouse/button`        | `{ "button": 1..5, "down": bool }`. |
| `POST`   | `/api/devices/:id/mouse/scroll`        | `{ "value": int, "horizontal": bool }`. |
| `POST`   | `/api/devices/ps4`                     | Create a DualShock 4. |
| `POST`   | `/api/devices/:id/ps4/state`           | `Ps4State` body, see `manager.rs`. |
| `POST`   | `/api/devices/ps5`                     | Create a DualSense. |
| `POST`   | `/api/devices/:id/ps5/state`           | `Ps5State` body. |
| `POST`   | `/api/devices/xone`                    | Create an Xbox One controller. |
| `POST`   | `/api/devices/:id/xone/state`          | `XOneState` body. |
| `WS`     | `/api/devices/:id/events`              | Event stream — see below. |

### WebSocket message types

Every message is JSON with a `type` field tag:

| `type`             | Fields |
| --- | --- |
| `hid_event`        | `ts_ms`, `kind` (`SET_FEATURE`/`WRITE_REPORT`/…), `report_id`, `data_hex` |
| `rumble`           | `ts_ms`, `left`, `right`, `left_trigger?`, `right_trigger?` |
| `led`              | `ts_ms`, `red`, `green`, `blue` |
| `player_led`       | `ts_ms`, `value` |
| `trigger_effect`   | `ts_ms`, `left_kind?`, `left_data_hex?`, `right_kind?`, `right_data_hex?` |
| `input_snapshot`   | `ts_ms`, `hex` (the bytes we just submitted) |
| `diag`             | `ts_ms`, `level`, `msg` |

Schema changes must update both the Rust serializer (`manager::DeviceEvent`)
and the JS handler in `frontend/app.js`. Make sure to bump
both ends in the same PR.

## Security

This server has **no auth by default**. The default bind is
`127.0.0.1:7878`, which restricts access to the local machine and is
fine for typical "drive my own host's virtual HID" usage.

For anything else there's an optional shared-secret gate:

```powershell
# token via flag…
cargo run --manifest-path web/Cargo.toml -- --addr 0.0.0.0:7878 --token s3cret

# …or via environment variable (preferred for systemd/Task Scheduler)
$env:WINUHID_WEB_TOKEN = "s3cret"
cargo run --manifest-path web/Cargo.toml -- --addr 0.0.0.0:7878
```

When `--token` is set, every `/api/*` REST call and the per-device
WebSocket must present the token in **either**:

* an `Authorization: Bearer <token>` header (preferred for REST), or
* a `?token=<token>` query parameter (the only option for WebSockets,
  since browsers can't set custom headers on a `WebSocket` URL).

Wrong or missing token → `401 Unauthorized`. The static frontend
(`/`, `/static/*`, `/favicon.ico`) is **always** reachable so users
can paste their token in.

### Loopback policy

The server enforces a loopback-vs-remote rule on startup:

| `--addr` resolves to… | `--token` unset | `--token` set |
| --- | --- | --- |
| Loopback (`127.0.0.1`, `::1`, `localhost`) | no auth (legacy default) | enforce |
| Anything else (`0.0.0.0`, LAN IP, hostname) | **refuse to start** | enforce |

If you try to bind to a non-loopback address without a token, the
server exits with:

> binding to a non-loopback address requires --token (or
> `WINUHID_WEB_TOKEN`). Anyone on the network would otherwise be able
> to create virtual HID devices on this machine.

### Passing the token from the browser

Open the UI with the token in the URL fragment:

```
http://host:7878/#token=s3cret
```

The page captures the token into `sessionStorage`, strips it from the
URL (so it doesn't end up in history or bookmarks), and attaches it
to every subsequent REST call (`Authorization` header) and to the
WebSocket URL (`?token=…` query parameter). The token survives
in-tab navigation but is dropped when the tab is closed.

For non-browser clients (curl, scripts), pass `Authorization: Bearer
<token>` directly. If you need stronger guarantees (TLS, audit logs,
per-user accounts), front the server with an authenticated reverse
proxy (e.g. Caddy with HTTP basic auth or mTLS) instead — the
built-in token gate is a single shared secret, not a full auth
system.

## Manual smoke test

After `just web-run` and opening the UI:

1. The header should show `driver: vN · devs: ok` (green).
2. Create a Mouse. Move the cursor with the *dx/dy* form — the system
   cursor should jump.
3. Create a DualShock 4. Drag the left stick — Windows
   *Set up USB game controllers* (`joy.cpl`) or
   `Settings → Bluetooth & devices → Devices` should report a
   "Wireless Controller". Press a face button and verify it lights up
   in `joy.cpl`.
4. Open Steam (or any game with rumble support) with the gamepad and
   trigger a rumble — the **Rumble L/R** bars should fill in real time.
5. Destroy the device. The OS should remove the controller within ~1s.

## Architecture map

* `src/main.rs`     — CLI, tracing, runtime, graceful shutdown.
* `src/ffi.rs`      — `WinUHid.dll` and `WinUHidDevs.dll` bindings.
* `src/manager.rs`  — device tracking, FFI callbacks, state types.
* `src/server.rs`   — axum router, WebSocket bridge, embedded frontend.
* `frontend/`       — HTML/CSS/JS, embedded into the binary at compile
  time by `rust-embed`.

When extending: keep all `extern "C"` declarations in `ffi.rs`. Keep
all blocking FFI work synchronous; `axum` handlers should never block
the runtime, but the existing handlers only do quick `lock-and-call`
sequences that are fine on the multi-threaded runtime.
