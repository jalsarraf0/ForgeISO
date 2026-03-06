# Runbook: Remote Agent

Author: Jamal Al-Sarraf

## Start agent on Linux host

```bash
cd agent
go run ./cmd/forgeiso-agent \
  --listen :7443 \
  --self-signed \
  --require-mtls \
  --job-token "$FORGEISO_AGENT_TOKEN"
```

## Client-side settings

Set in ForgeISO config (`remote_agent`):
- `enabled: true`
- `endpoint: https://<agent-host>:7443`
- `job_token: <token>`
- `ca_cert`, `client_cert`, `client_key` for mTLS

## Validation

- Submit a build/test job from GUI or CLI-integrated flow
- Stream logs via `StreamBuildLogs`/`StreamTestLogs`
- Fetch artifacts after completion with `FetchArtifacts`
