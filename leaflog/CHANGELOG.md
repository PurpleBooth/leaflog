# Changelog

All notable changes to the Leaflog Home Assistant app are documented
here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.1.0] - 2026-11-08

### Added

- Initial milestone (M1): workspace skeleton, ingress-correct SPA
  serving with a runtime-injected base path, `/api/health`, connection
  gating to the Supervisor ingress proxy, s6 service packaging, and
  multi-arch image build (amd64, aarch64).
- Mock Home Assistant Supervisor for local development.
