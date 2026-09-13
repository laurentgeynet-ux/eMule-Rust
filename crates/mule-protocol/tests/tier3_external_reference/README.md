# Tier 3 External Reference Orchestration Harness

This module implements the **Tier 3 External Reference Orchestration Adapter** for Emule-Sion.

## Official Project Attestation Statement

> **« Compatibilité eD2K expérimentale validée sur corpus statique et pair de référence émulé, en environnement loopback fermé. Le harnais Tier 3 pour un binaire eMule/aMule externe épinglé est disponible, mais son test live reste ignoré par défaut tant qu’un binaire vérifié n’est pas fourni. »**

## Attestation Levels

To ensure absolute engineering rigor and honest claims of interoperability, the test suite is partitioned into three distinct tiers:

| Tier | Attestation Boundary | What is Proved | What is NOT Proved |
|---|---|---|---|
| **Tier 1: Static Corpus** | Static byte fixtures (`tests/fixtures/ed2k/*.bin`) | Syntactic correctness and deterministic invariance of decoders/encoders on frozen wire frames. | Compatibility with any live running process. |
| **Tier 2: Emulated Reference Peer** | `EmulatedReferencePeer` (`crates/mule-protocol/src/ed2k/reference.rs`) | Interoperability against our in-process Rust model derived from eMule 0.50a / aMule 2.3.3 source code. | Real binary execution or runtime quirks of compiled C++ codebases. |
| **Tier 3: External Reference Orchestration** | Out-of-process orchestrator for pinned real external binary (`tests/tier3_external_reference/`) | Live interoperability against compiled, pinned reference binaries (eMule 0.50a or aMule 2.3.3) over closed local loopback. | Public network operation or unverified binary builds. |

> [!IMPORTANT]
> **No live external interoperability is claimed** until Tier 3 tests have been explicitly executed against a pinned reference binary. Tier 2 verifies conformity to our mental and source-derived model; only Tier 3 verifies conformity to an actual external executable.

## Mandatory Tier 3 Pre-Execution Checklist

Before triggering any live test against an external reference binary, all 10 conditions must be satisfied:

1. **Approved Local Storage**: The reference binary is stored locally in an approved test directory outside temporary folders.
2. **Manifest Declaration**: `reference_manifest.json` declares the project, version, architecture, expected SHA-256, compatibility profile, launch options, and known limitations.
3. **Cryptographic Validation**: The harness calculates the SHA-256 fingerprint immediately prior to launch and aborts before `spawn()` if the digest does not match.
4. **Hermetic Neutralization**: The ephemeral configuration explicitly disables Kademlia (`EnableKad=0`), server auto-connect (`AutoConnect=0`), server list downloads (`AutoUpdateServerList=0`), update notifications, UPnP, external IP discovery, and public web interfaces.
5. **Strict Loopback Binding**: Every socket address is mechanically asserted as `127.0.0.1` or `::1` before any connection attempt.
6. **Dynamic Ephemeral Ports**: TCP and UDP ports are dynamically allocated by the OS via loopback binding (`127.0.0.1:0`) and never hardcoded.
7. **Sequential Execution**: The live test suite must be run sequentially with `--test-threads=1` to prevent port or process race conditions.
8. **Short Bounded Timeouts**: Launch, handshake, read, and transfer timeouts are bounded and short (e.g. 500ms - 3s).
9. **RAII Process Lifecycle**: All child processes are strictly reaped via RAII `Drop` guards (`child.kill()` and `child.wait()`) on success, failure, panic, or timeout.
10. **Automated Anonymity Audit**: All wire traces and divergence reports must pass automated anonymity validation before being written or attached to CI artifacts.

## Explicit Compatibility Profile in Traces & Reports

To prevent ambiguous interpretations of overloaded opcodes (such as `0x47` vs `0x4B` or `0x4F` vs `0x49`), every trace record and divergence report must explicitly declare its `compatibilityProfile`:

```json
{
  "compatibilityProfile": "Emule050a",
  "protocolByte": "0xE3",
  "opcode": "0x47",
  "direction": "client_to_peer",
  "payloadLength": 40,
  "outcome": "accepted"
}
```

## Running the Tier 3 Test Suite

Tier 3 tests are **disabled by default** (`#[ignore]`) and feature-gated behind `ed2k-reference-loopback`.

```bash
# Non-ignored unit tests of Tier 3 harness, environment and reports:
cargo test -p mule-protocol --test tier3_loopback --features ed2k-reference-loopback

# Live test (requires pinned binary via environment variable):
$env:EMULE_REFERENCE_BIN = "C:\path\to\emule.exe"
$env:EMULE_REFERENCE_SHA256 = "expected_sha256_hex"
cargo test -p mule-protocol --test tier3_loopback --features ed2k-reference-loopback -- --ignored --test-threads=1
```
