# ADR 0004 : Connectivité eD2K/Kad Publique en Production

## Statut
Accepté

## Date
2026-03-03

## Contexte
L'[ADR 0002](0002-no-public-network.md) interdisait toute connectivité réseau externe pendant la phase de construction : parseurs non éprouvés, risque d'exposition précoce. Cette phase est terminée : les codecs sont couverts par les vecteurs Tier 1, les pairs émulés Tier 2, et surtout les **binaires de référence réels Tier 3** (eMule 0.50a, eMule 0.72a, aMule 3.1.0 en loopback sandboxé). L'application passe en version de production — un client eD2K sans accès au réseau réel n'a pas de valeur fonctionnelle.

## Décision
1. **Le profil `public-ed2k-kad-preview` du daemon est autorisé en production** : connexions sortantes vers serveurs eD2K publics, pairs eD2K et bootstrap Kad sont permises quand ce profil est actif.
2. **Le défaut reste confiné** : le flavor `standard` démarre sans connexion automatique ; la connectivité publique requiert une action utilisateur explicite (profil réseau / connexion serveur). Aucun build public n'expose les méthodes `lab.*` ni ne compile `lab-loopback`/`lab-tools`.
3. **La pureté de `mule-protocol` est maintenue** : zéro socket, zéro I/O réseau dans la crate — la connectivité vit dans `mule-daemon` (inbound listener, peer sessions, `kad_udp`) sous le contrôle de `mule-network-policy` (anti-SSRF, budgets, kill switch).
4. **Suite de tests hors-ligne par défaut** : `cargo test` ne doit jamais émettre de trafic sortant. Les tests réseau réel restent `#[ignore]`d derrière des variables d'environnement explicites (ex. `EMULE_REFERENCE_BIN` pour le Tier 3 externe) — jamais activés en CI standard.
5. **Tests réseau réel (Phase D)** : exécutés manuellement, supervisés, sur données jetables, avec collecte de métriques anonymisées ; ils ne sont pas reproductibles en CI par nature.

## Conséquences
- **Positives** : le produit devient fonctionnel comme client eD2K/Kad ; validation réelle de l'interopérabilité ; comportement réseau observé et testé sur le vrai protocole.
- **Négatives** : exposition aux pairs non authentifiés du réseau public — mitigée par les bornes de `limits.rs`, le pipeline anti-SSRF, le kill switch atomique, les budgets de ressources et le fuzzing des parseurs.
- **Dette assumée** : le profil `public-ed2k-kad-preview` garde son suffixe « preview » jusqu'à la validation complète du plan de test `docs/qa/PLAN_TEST_COMPLET.md` (Phases A–D).
