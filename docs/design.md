# ForgeISO Architecture

## Current model

ForgeISO is a Linux-only, bare-metal ISO remastering tool.

All product workflows run locally through the Rust engine:
- CLI for automation
- TUI for terminal operators
- GUI for desktop users

There is no product-side server process, remote agent, or container runtime dependency.

## Engine responsibilities

- Resolve an ISO source from a local path or user-provided URL
- Inspect the ISO and detect distro metadata from the image itself
- Validate local host prerequisites
- Extract and repack supported Linux ISO layouts with local tools
- Apply local overlay content into the ISO or unpacked rootfs
- Run local scan, test, and report steps
- Emit structured events for UI logging

## CI/CD boundary

CI may still use ephemeral containers for repeatable pipeline stages. Those containers are not part of the shipped product workflow.
