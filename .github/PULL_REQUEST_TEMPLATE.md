## Résumé

<!-- Quoi et pourquoi — référencer l'issue liée (Fixes #123) -->

## Type de changement

- [ ] Correction de bug
- [ ] Nouvelle fonctionnalité
- [ ] Refactoring / chore
- [ ] Documentation
- [ ] Changement protocolaire filaire (eD2K/Kad) ⚠️

## Checklist

- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` passe
- [ ] `cargo test --workspace --all-features` passe
- [ ] `flutter analyze` + `flutter test` passent (`apps/desktop`)
- [ ] Aucun test n'émet de trafic réseau sortant (loopback uniquement)
- [ ] Code et documentation en français
- [ ] Si touche au filaire : comportement vérifié contre eMule 0.72a
      (`srchybrid`) + spec `docs/ed2k/` + vecteur de test ajouté
- [ ] Si touche à la sécurité : pipeline anti-SSRF / isolation loopback /
      kill switch / pseudonymisation non affaiblis
- [ ] Aucune fonctionnalité `lab.*` exposée dans le flavor `standard`
- [ ] Nouvelle dépendance : compatible `deny.toml` et préférablement
      déclarée dans `[workspace.dependencies]`
