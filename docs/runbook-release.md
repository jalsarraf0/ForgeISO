# Runbook: Linux Release Artifacts

Release artifacts are Linux-only.

Current packaged outputs are intended for bare-metal local use and should contain the ForgeISO CLI and TUI binaries. GUI packaging is handled through Tauri's Linux bundle targets.

CI container jobs remain ephemeral and must be torn down after the pipeline finishes.
