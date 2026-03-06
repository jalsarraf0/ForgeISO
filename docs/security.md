# Security Notes

## Local-first execution

ForgeISO performs product workflows locally on the host machine. It does not open product-side network services or require a remote agent.

## Host trust boundary

- Inspect the source ISO before building
- Use only local overlay directories you trust
- Review generated reports before distributing a remastered image
- Keep local toolchain packages current on the Linux host

## CI/CD containers

CI containers are allowed only for pipeline execution. They should be ephemeral and removed when the pipeline completes.
