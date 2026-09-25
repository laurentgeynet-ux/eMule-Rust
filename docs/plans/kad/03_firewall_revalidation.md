# Étape 3 — Revalidation firewall UDP

> **Dépendances** : aucune. Petite étape, gros gain UX — livrable en quick win dès le début.
> **Risque interop** : nul — le cycle `FWCHECKUDPREQ → FIREWALLUDP` existe déjà ; on change seulement **quand** on le relance.

## Objectif

Aujourd'hui la maintenance n'exécute le test que tant que `!fw.verified` (`kad.rs:694-705`) : une fois `Open` ou `Firewalled` conclu, l'état est **figé jusqu'au redémarrage** — un changement de NAT/IP n'est jamais détecté.

## État vérifié

`FirewallTestTracker` (`types.rs:161-180`) a déjà : `status`, `verified`, `extern_port`, `use_extern`, `observed_ip`, `last_check`, `pending`.

## Changements

### 3a. `kad_udp/types.rs`

- `FirewallTestTracker::mark_stale()` — remet `verified = false`, `status = Unverified`, conserve `observed_ip`/`extern_port` (servent à détecter le changement).
- Champ `last_observed_ip: Option<Ipv4Addr>` séparé de `observed_ip` si nécessaire pour comparer « IP observée au dernier test » vs « IP observée dans le dernier `FIREWALLED_RES` reçu ».

### 3b. Déclencheurs de revalidation (`kad.rs` maintenance ~l.694)

```rust
let needs_fw_check = {
    let fw = tracker.lock().await;
    (!fw.verified
        || fw.last_check.map(|t| t.elapsed() > FW_RECHECK_INTERVAL).unwrap_or(false)
        || fw.observed_ip_changed())
        && fw.status != UdpFirewallStatus::Testing
};
```

| Déclencheur | Source |
|---|---|
| `verified=false` | comportement actuel |
| `last_check` expiré | `FW_RECHECK_INTERVAL` (const `4 h` en tête de `firewall.rs`) |
| IP observée différente | `handlers.rs` — sur réception `FIREWALLED_RES`/`FIREWALLUDP` avec IP ≠ `last_observed_ip` → `mark_stale()` |
| Changement de bind/port | hook `KadUdpService::start`/`restart` → `mark_stale()` |

### 3c. `kad/firewall.rs`

`run_udp_firewall_check` inchangé fonctionnellement ; documenter qu'il peut désormais être relancé sur un tracker `verified` mais `stale`.

## Tests

- Tracker : `mark_stale` préserve `observed_ip`, remet `verified`.
- Simuler un `FIREWALLED_RES` avec IP différente → `needs_fw_check` vrai.
- `last_check` > intervalle → `needs_fw_check` vrai sans autre changement.

## Critères de fin

- [ ] L'état converge après un changement d'IP observée **sans redémarrage**.
- [ ] Retest périodique seulement après `FW_RECHECK_INTERVAL` — pas de boucle.
- [ ] `Testing` n'est jamais interrompu par un déclencheur concurrent.

## Référence eMule

`CUDPFirewallTester` — eMule re-teste sur changement de conditions réseau (reconnexion, nouvelle IP publique).
