# Politique de Sécurité — EMule-Rust

## Versions supportées

| Version | Support |
| :--- | :--- |
| `0.2.x` (dernière release) | ✅ Correctifs de sécurité |
| `< 0.2.0` / `-lab` / `-RC` | ❌ Non supportées |

## Surface de sécurité

EMule-Rust est un client P2P exposé au réseau. Les mécanismes défensifs
suivants sont **non négociables** et toute régression y afférente est
traitée comme un incident de sécurité :

- Pipeline anti-SSRF en 7 étapes (`mule-network-policy`)
- Isolation loopback par défaut et kill switch atomique
- Filtrage IP (`ipfilter.dat`, `mule-filter`)
- Pseudonymisation salée des pairs
- Isolation des fonctionnalités de laboratoire derrière la feature
  `lab-tools` — jamais présentes dans les builds publics `standard`

## Signaler une vulnérabilité

**Ne pas ouvrir d'issue publique** pour une vulnérabilité non divulguée.

Utiliser de préférence **GitHub Security Advisories** (onglet
*Security* → *Report a vulnerability*) sur le dépôt, ou contacter le
mainteneur par e-mail : **Loulach**
<Dev.iptvmediacenter@gmail.com>.

Inclure si possible :

- Version concernée (`version.json` ou tag)
- Composant (`mule-daemon`, `mule-protocol`, UI…)
- Description de l'impact et scénario d'exploitation
- Reproduction ou preuve de concept (environnement hors-ligne/loopback
  de préférence)

### Délais indicatifs

- Accusé de réception : sous 7 jours
- Évaluation et plan de correction : sous 30 jours
- Divulgation coordonnée après publication du correctif

## Consommation de sécurité en amont

- `deny.toml` (cargo-deny) : CVE/RustSec en `deny`, licences restreintes
- SBOM CycloneDX générée à chaque release (`sbom/` du bundle)
- `SHA256SUMS.txt` signé par build pour vérification d'intégrité
