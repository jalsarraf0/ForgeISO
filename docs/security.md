# Security Baseline

Author: Jamal Al-Sarraf

## Defaults

- Containerized execution for build/test/scan stages
- `dangerous_mode` disabled by default
- SSH hardening defaults:
  - `PermitRootLogin no`
  - `PasswordAuthentication no`
  - `PubkeyAuthentication yes`
- Secrets scanning enabled by default
- Vulnerability gate defaults to `CRITICAL`

## Scan outputs

- SBOM artifacts:
  - SPDX JSON
  - CycloneDX JSON
- Vulnerability reports:
  - Trivy filesystem scan
  - Optional Syft/Grype reports
- Compliance:
  - OpenSCAP stage output
- Secrets:
  - Pattern-based workspace findings report

## Remote agent hardening

- TLS required for all gRPC traffic
- mTLS enabled by default
- Job token verification via request metadata (`x-job-token`)
- Artifact/log retrieval scoped per job ID

## Recommended enterprise controls

- Pin container images by digest in production
- Store checksums/SBOMs in immutable artifact storage
- Run CI on isolated runners with hardened Docker daemon policy
- Rotate agent tokens and client certificates regularly
