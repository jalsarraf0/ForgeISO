#!/usr/bin/env python3
"""Optional hooks runner for ForgeISO.

Disabled by default. Host command execution requires explicit --allow-host-exec.
"""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from pathlib import Path
from typing import Any


def load_hook(path: Path) -> dict[str, Any]:
    data = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(data, dict):
        raise ValueError("hook file must be a JSON object")
    return data


def run_transform(hook: dict[str, Any], allow_host_exec: bool) -> dict[str, Any]:
    result: dict[str, Any] = {"status": "ok", "messages": []}

    if "set_env" in hook:
        env = hook["set_env"]
        if not isinstance(env, dict):
            raise ValueError("set_env must be a dict")
        for key, value in env.items():
            os.environ[str(key)] = str(value)
            result["messages"].append(f"set env {key}")

    if "command" in hook:
        if not allow_host_exec:
            raise PermissionError(
                "hook command requested but host execution is disabled; pass --allow-host-exec"
            )

        cmd = hook["command"]
        if not isinstance(cmd, list) or not all(isinstance(item, str) for item in cmd):
            raise ValueError("command must be a list of strings")

        proc = subprocess.run(cmd, check=False, capture_output=True, text=True)
        result["command"] = {
            "argv": cmd,
            "exit_code": proc.returncode,
            "stdout": proc.stdout,
            "stderr": proc.stderr,
        }
        if proc.returncode != 0:
            result["status"] = "failed"

    return result


def main() -> int:
    parser = argparse.ArgumentParser(description="ForgeISO optional hook runner")
    parser.add_argument("hook", type=Path, help="path to hook json file")
    parser.add_argument("--allow-host-exec", action="store_true", help="allow subprocess execution")
    args = parser.parse_args()

    try:
        hook = load_hook(args.hook)
        result = run_transform(hook, args.allow_host_exec)
    except Exception as exc:  # noqa: BLE001
        print(json.dumps({"status": "error", "error": str(exc)}, indent=2))
        return 1

    print(json.dumps(result, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
