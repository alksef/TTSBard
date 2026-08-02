#!/usr/bin/env python3
"""Keep a VTube Studio WebSocket open and report exactly when it breaks.

The probe doesn't authenticate and doesn't modify VTube Studio state. It sends
APIStateRequest periodically, which is allowed before authentication, and logs
responses, latency, WebSocket close frames, and underlying socket errors.
"""

from __future__ import annotations

import argparse
import asyncio
import json
import logging
import os
import sys
import time
import uuid
from pathlib import Path
from typing import Any

import websockets
from websockets.exceptions import ConnectionClosed


LOG = logging.getLogger("vtube-ws-probe")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Hold a VTube Studio WebSocket open and diagnose disconnects."
    )
    parser.add_argument(
        "--url",
        default="ws://127.0.0.1:8001",
        help="VTube Studio Plugin API URL (default: %(default)s)",
    )
    parser.add_argument(
        "--interval",
        type=float,
        default=30.0,
        help="Seconds between APIStateRequest probes (default: %(default)s)",
    )
    parser.add_argument(
        "--duration",
        type=float,
        default=0.0,
        help="Stop cleanly after this many seconds; 0 runs until Ctrl+C",
    )
    parser.add_argument(
        "--open-timeout",
        type=float,
        default=10.0,
        help="Connection timeout in seconds (default: %(default)s)",
    )
    parser.add_argument(
        "--ws-ping-interval",
        type=float,
        default=0.0,
        help=(
            "Send WebSocket protocol pings at this interval; 0 disables client "
            "pings while still answering server pings"
        ),
    )
    parser.add_argument(
        "--authenticate",
        action="store_true",
        help=(
            "Authenticate with the saved TTSBard token. The token is read from "
            "settings and is never printed"
        ),
    )
    parser.add_argument(
        "--settings-file",
        type=Path,
        help=(
            "TTSBard settings.json used by --authenticate "
            "(default: %%APPDATA%%/ttsbard/settings.json)"
        ),
    )
    parser.add_argument(
        "--plugin-name",
        default="TTSBard",
        help="Plugin name used for authentication (default: %(default)s)",
    )
    parser.add_argument(
        "--plugin-developer",
        default="TTSBard",
        help="Plugin developer used for authentication (default: %(default)s)",
    )
    parser.add_argument(
        "--log-file",
        type=Path,
        help="Also append logs to this UTF-8 file",
    )
    parser.add_argument(
        "--protocol-debug",
        action="store_true",
        help="Log WebSocket frames, including automatic PING/PONG handling",
    )
    args = parser.parse_args()

    if args.interval <= 0:
        parser.error("--interval must be greater than zero")
    if args.duration < 0:
        parser.error("--duration cannot be negative")
    if args.open_timeout <= 0:
        parser.error("--open-timeout must be greater than zero")
    if args.ws_ping_interval < 0:
        parser.error("--ws-ping-interval cannot be negative")
    return args


def configure_logging(log_file: Path | None, protocol_debug: bool) -> None:
    formatter = logging.Formatter(
        fmt="%(asctime)s.%(msecs)03d %(levelname)s %(message)s",
        datefmt="%Y-%m-%dT%H:%M:%S",
    )
    stream = logging.StreamHandler()
    stream.setFormatter(formatter)
    LOG.addHandler(stream)

    if log_file is not None:
        log_file.parent.mkdir(parents=True, exist_ok=True)
        file_handler = logging.FileHandler(log_file, encoding="utf-8")
        file_handler.setFormatter(formatter)
        LOG.addHandler(file_handler)

    LOG.setLevel(logging.INFO)

    if protocol_debug:
        protocol_logger = logging.getLogger("websockets.client")
        protocol_logger.handlers = list(LOG.handlers)
        protocol_logger.propagate = False
        protocol_logger.setLevel(logging.DEBUG)


def api_state_request(request_id: str) -> str:
    return json.dumps(
        {
            "apiName": "VTubeStudioPublicAPI",
            "apiVersion": "1.0",
            "requestID": request_id,
            "messageType": "APIStateRequest",
        },
        separators=(",", ":"),
    )


def authentication_request(
    request_id: str, token: str, plugin_name: str, plugin_developer: str
) -> str:
    return json.dumps(
        {
            "apiName": "VTubeStudioPublicAPI",
            "apiVersion": "1.0",
            "requestID": request_id,
            "messageType": "AuthenticationRequest",
            "data": {
                "pluginName": plugin_name,
                "pluginDeveloper": plugin_developer,
                "authenticationToken": token,
            },
        },
        separators=(",", ":"),
    )


def resolve_settings_file(configured: Path | None) -> Path:
    if configured is not None:
        return configured
    appdata = os.environ.get("APPDATA")
    if not appdata:
        raise RuntimeError(
            "APPDATA is unavailable; pass --settings-file explicitly"
        )
    return Path(appdata) / "ttsbard" / "settings.json"


def load_saved_token(settings_file: Path) -> str:
    try:
        settings = json.loads(settings_file.read_text(encoding="utf-8-sig"))
    except OSError as exc:
        raise RuntimeError(f"cannot read settings file {settings_file}: {exc}") from exc
    except json.JSONDecodeError as exc:
        raise RuntimeError(f"invalid JSON in settings file {settings_file}: {exc}") from exc

    vtube_settings = settings.get("vtube_studio")
    token = vtube_settings.get("token") if isinstance(vtube_settings, dict) else None
    if not isinstance(token, str) or not token:
        raise RuntimeError(f"no VTube Studio token found in {settings_file}")
    return token


async def authenticate(
    websocket: Any,
    token: str,
    plugin_name: str,
    plugin_developer: str,
) -> None:
    request_id = f"probe-auth-{uuid.uuid4()}"
    started_at = time.monotonic()
    protocol_logger = logging.getLogger("websockets.client")
    protocol_debug_was_disabled = protocol_logger.disabled
    protocol_logger.disabled = True
    try:
        await websocket.send(
            authentication_request(request_id, token, plugin_name, plugin_developer)
        )
        LOG.info(
            "send type=AuthenticationRequest request_id=%s plugin_name=%r "
            "plugin_developer=%r token_present=true",
            request_id,
            plugin_name,
            plugin_developer,
        )

        while True:
            raw = await asyncio.wait_for(websocket.recv(), timeout=12)
            if isinstance(raw, bytes):
                LOG.info("recv during authentication binary message bytes=%d", len(raw))
                continue
            try:
                message = json.loads(raw)
            except json.JSONDecodeError:
                LOG.info("recv during authentication non-JSON text chars=%d", len(raw))
                continue

            if str(message.get("requestID")) != request_id:
                LOG.info(
                    "recv during authentication unrelated type=%s request_id=%s",
                    message.get("messageType", "?"),
                    message.get("requestID", "?"),
                )
                continue

            message_type = message.get("messageType")
            data = message.get("data")
            latency_ms = (time.monotonic() - started_at) * 1000
            if message_type == "AuthenticationResponse" and isinstance(data, dict):
                authenticated = data.get("authenticated") is True
                LOG.info(
                    "recv type=AuthenticationResponse request_id=%s latency_ms=%.1f "
                    "authenticated=%s reason=%r",
                    request_id,
                    latency_ms,
                    authenticated,
                    data.get("reason"),
                )
                if not authenticated:
                    raise RuntimeError("VTube Studio rejected the saved authentication token")
                return
            if message_type == "APIError" and isinstance(data, dict):
                raise RuntimeError(
                    "VTube Studio authentication API error "
                    f"{data.get('errorID')}: {data.get('message')}"
                )
            raise RuntimeError(
                f"unexpected authentication response type: {message_type!r}"
            )
    finally:
        protocol_logger.disabled = protocol_debug_was_disabled


def exception_chain(exc: BaseException) -> str:
    parts: list[str] = []
    seen: set[int] = set()
    current: BaseException | None = exc
    while current is not None and id(current) not in seen:
        seen.add(id(current))
        details = f"{type(current).__name__}: {current}"
        winerror = getattr(current, "winerror", None)
        errno = getattr(current, "errno", None)
        if winerror is not None:
            details += f" (winerror={winerror})"
        elif errno is not None:
            details += f" (errno={errno})"
        parts.append(details)
        current = current.__cause__ or current.__context__
    return " <- ".join(parts)


def describe_response(raw: str | bytes, pending: dict[str, float]) -> str:
    if isinstance(raw, bytes):
        return f"binary message bytes={len(raw)}"

    try:
        message: dict[str, Any] = json.loads(raw)
    except (json.JSONDecodeError, TypeError):
        return f"non-JSON text chars={len(raw)}"

    message_type = message.get("messageType", "?")
    request_id = message.get("requestID", "?")
    sent_at = pending.pop(str(request_id), None)
    latency = ""
    if sent_at is not None:
        latency = f" latency_ms={(time.monotonic() - sent_at) * 1000:.1f}"

    data = message.get("data")
    if message_type == "APIStateResponse" and isinstance(data, dict):
        return (
            f"recv type={message_type} request_id={request_id}{latency} "
            f"active={data.get('active')} "
            f"authenticated={data.get('currentSessionAuthenticated')} "
            f"vts_version={data.get('vTubeStudioVersion')}"
        )
    if message_type == "APIError" and isinstance(data, dict):
        return (
            f"recv type=APIError request_id={request_id}{latency} "
            f"error_id={data.get('errorID')} message={data.get('message')!r}"
        )
    return f"recv type={message_type} request_id={request_id}{latency}"


async def run_probe(args: argparse.Namespace) -> int:
    ping_interval = args.ws_ping_interval or None
    started_at = time.monotonic()
    pending: dict[str, float] = {}
    auth_token: str | None = None

    if args.authenticate:
        settings_file = resolve_settings_file(args.settings_file)
        auth_token = load_saved_token(settings_file)
        LOG.info(
            "authentication enabled settings_file=%s token_present=true",
            settings_file,
        )

    LOG.info(
        "connecting url=%s interval_s=%s duration_s=%s ws_ping_interval=%s",
        args.url,
        args.interval,
        args.duration,
        ping_interval,
    )

    try:
        async with websockets.connect(
            args.url,
            open_timeout=args.open_timeout,
            close_timeout=5,
            ping_interval=ping_interval,
            ping_timeout=10 if ping_interval is not None else None,
            max_size=1024 * 1024,
        ) as websocket:
            LOG.info(
                "connected local=%s remote=%s subprotocol=%s",
                websocket.local_address,
                websocket.remote_address,
                websocket.subprotocol,
            )

            if auth_token is not None:
                await authenticate(
                    websocket,
                    auth_token,
                    args.plugin_name,
                    args.plugin_developer,
                )

            next_probe = time.monotonic()
            while True:
                now = time.monotonic()
                elapsed = now - started_at
                if args.duration and elapsed >= args.duration:
                    LOG.info("duration complete elapsed_s=%.3f", elapsed)
                    return 0

                if now >= next_probe:
                    request_id = f"probe-{uuid.uuid4()}"
                    pending[request_id] = now
                    await websocket.send(api_state_request(request_id))
                    LOG.info(
                        "send type=APIStateRequest request_id=%s elapsed_s=%.3f",
                        request_id,
                        elapsed,
                    )
                    next_probe = now + args.interval

                wait_for = max(0.01, next_probe - time.monotonic())
                if args.duration:
                    remaining = args.duration - (time.monotonic() - started_at)
                    wait_for = max(0.01, min(wait_for, remaining))

                try:
                    raw = await asyncio.wait_for(websocket.recv(), timeout=wait_for)
                except TimeoutError:
                    continue
                LOG.info("%s", describe_response(raw, pending))

    except ConnectionClosed as exc:
        LOG.error(
            "websocket closed code=%s reason=%r elapsed_s=%.3f details=%s",
            exc.code,
            exc.reason,
            time.monotonic() - started_at,
            exception_chain(exc),
        )
        return 2
    except (OSError, RuntimeError, TimeoutError, websockets.WebSocketException) as exc:
        LOG.error(
            "connection failed elapsed_s=%.3f details=%s",
            time.monotonic() - started_at,
            exception_chain(exc),
        )
        return 2


def main() -> int:
    args = parse_args()
    configure_logging(args.log_file, args.protocol_debug)
    try:
        return asyncio.run(run_probe(args))
    except KeyboardInterrupt:
        LOG.info("stopped by user")
        return 0


if __name__ == "__main__":
    sys.exit(main())
