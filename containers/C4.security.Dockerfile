FROM debian:bookworm-slim

ARG TRIVY_VERSION=v0.69.3
ARG SYFT_VERSION=v1.42.1
ARG GRYPE_VERSION=v0.109.0

RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    ca-certificates \
    tar \
    gzip \
    git \
    && rm -rf /var/lib/apt/lists/*

RUN set -eux; \
    fetch_script() { \
      url="$1"; \
      out="$2"; \
      attempt=1; \
      while [ "${attempt}" -le 5 ]; do \
        if curl -fsSL --retry 3 --retry-all-errors --retry-delay 2 "${url}" -o "${out}"; then \
          return 0; \
        fi; \
        attempt="$((attempt + 1))"; \
        sleep 2; \
      done; \
      return 1; \
    }; \
    fetch_script "https://raw.githubusercontent.com/aquasecurity/trivy/${TRIVY_VERSION}/contrib/install.sh" /tmp/install-trivy.sh; \
    sh /tmp/install-trivy.sh -b /usr/local/bin "${TRIVY_VERSION}"; \
    fetch_script "https://raw.githubusercontent.com/anchore/syft/${SYFT_VERSION}/install.sh" /tmp/install-syft.sh; \
    sh /tmp/install-syft.sh -b /usr/local/bin "${SYFT_VERSION}"; \
    if fetch_script "https://raw.githubusercontent.com/anchore/grype/${GRYPE_VERSION}/install.sh" /tmp/install-grype.sh \
      && sh /tmp/install-grype.sh -b /usr/local/bin "${GRYPE_VERSION}"; then \
      echo "Installed grype ${GRYPE_VERSION}"; \
    else \
      echo "WARNING: grype installer unavailable, continuing without grype"; \
    fi; \
    rm -f /tmp/install-trivy.sh /tmp/install-syft.sh /tmp/install-grype.sh

WORKDIR /workspace
