# Release Notes — eMuleRust v0.2.0-RC3

**Release date:** October 2026
**Status:** Release Candidate — full **eMuleRust** rebrand, A–D
validation campaign fully green, anti-SSRF hardening, first public
GitHub repository.

> 🇫🇷 Version française : [RELEASE_NOTES_0.2.0-RC3.fr.md](RELEASE_NOTES_0.2.0-RC3.fr.md)

---

## 1. Release Summary

**v0.2.0-RC3** is the first release published under the final name
**eMuleRust** on GitHub (`laurentgeynet-ux/eMule-Rust`). It builds on
RC2 (exhaustive eMule 0.72a wire audit + live interoperability) and
adds a complete 4-gate validation campaign — all green.

1. **eMuleRust rebrand** — display name, executables
   (`eMuleRust.exe`), bundles, `EMULERUST_*` environment variables
   (deprecated `EMULE_SION_*` aliases kept), Dart/Rust identifiers,
   bilingual docs (English README + `README.fr.md`).

2. **A–D validation campaign**:
   - **Phase A**: `verify_all.ps1` 100% green, standard bundle
     rebuilt and smoke-tested (no `lab.*` method exposed).
   - **Phase B**: functional RPC bench **134 PASS / 0 FAIL** —
     full public surface coverage (identity, downloads,
     `.met`/`.dat` formats, shares, sources, servers, Kad, filters,
     settings, history) + error paths.
   - **Phase C**: live interop **7/7 × 3 references** — eMule 0.50a,
     eMule 0.72a and aMule 3.1.0 (SHA-256-pinned binaries,
     hermetic sandbox, strict loopback).
   - **Phase D**: supervised real-network bench **21/21** — real
     `server.met` import (16 servers), eD2K session established,
     95 Kad contacts, real eD2K + Kad searches, kill switch,
     UPnP mapped then cleanly unmapped (< 500 ms).

3. **Anti-SSRF hardening**: `servers.updateFromUrl` and
   `kad.bootstrapFromUrl` now validate scheme (http/https only) and
   resolved addresses — link-local, private LAN and policy-forbidden
   targets rejected.

4. **Robustness fixes**:
   - Bounded daemon shutdown: detached UPnP unmap (`std::thread` +
     500 ms oneshot) — no more > 5 s stalls on slow gateways.
   - Packaged smoke test: continuous daemon pipe draining.
   - Loopback E2E: deterministic crash/resume via `downloads.pause`.
   - Test fixtures consolidated into `mule-test-support`.

5. **GitHub CI/CD**: `CI` workflow (Rust check/clippy/test +
   Flutter analyze/test, daemon compiled for supervisor/E2E tests),
   `Packages` workflow (Windows + Linux bundles as artifacts),
   `release.yml` (build + smoke + GitHub Release on `v*` tags),
   Dependabot enabled.

## 2. Validation Commands

```powershell
# Full verification
powershell -NoProfile -ExecutionPolicy RemoteSigned -File scripts\verify_all.ps1

# Functional RPC bench (loopback, offline)
powershell -NoProfile -ExecutionPolicy RemoteSigned -File scripts\test_phase_b_functional_bench.ps1

# Real-network bench (supervised — emits public eD2K/Kad traffic)
powershell -NoProfile -ExecutionPolicy RemoteSigned -File scripts\test_phase_d_public_network.ps1

# Live interop vs pinned reference binaries
cargo test -p mule-protocol --test tier3_loopback --all-features -- --include-ignored
```

## 3. Migrations

- **Environment variables**: `EMULERUST_WORKDIR`,
  `EMULERUST_DOWNLOADS_DIR`, `EMULERUST_DEPLOY_DIR` take precedence;
  legacy `EMULE_SION_*` / `EMULE_DEPLOY_DIR` are still read as
  deprecated fallbacks (no action needed).
- **UPnP**: leases created under the legacy `eMule-Sion` prefix are
  still recognized and unmapped on shutdown.
- **Workdir**: `%LOCALAPPDATA%\eMuleRust` is used; no automatic
  migration from `eMule-Sion` — re-import via the eMule migration
  wizard if needed.

## 4. Security Notes

- Anti-SSRF applied to URL fetches (server.met / nodes.dat) —
  scheme + DNS resolution checked against the network policy.
- No `lab.*` method in the `standard` flavor — verified by
  `test_packaged_bundle.ps1` in CI.
- `master` branch protected: force-push and deletion blocked.
- Unsigned builds (optional Authenticode via
  `WINDOWS_CERT_BASE64` / `WINDOWS_CERT_PASSWORD` secrets).

## 5. Known Limitations

- **Windows x64 only** for the packaged UI (the Linux CI bundle is
  experimental — first automated build added in this RC).
- eD2K session observed as **LowID** in the Phase D bench despite a
  confirmed UPnP mapping — acceptable (degraded) behavior, under
  investigation.
- No background downloads on Android (OS constraint) — the Android
  target is not maintained.
