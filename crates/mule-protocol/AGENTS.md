# AGENTS.md — mule-protocol (règles spécifiques)

> Ces règles complètent le [AGENTS.md racine](../../AGENTS.md) et priment ici.

`mule-protocol` est la crate la plus sensible du workspace : elle définit le cadrage, les codecs et les limites du protocole filaire eD2K/Kad. Toute erreur ici casse l'interopérabilité avec eMule 0.72a ou ouvre une faille de parsing.

**Référence de vérité** : sources eMule 0.72a dans `D:\eMule0.72a-Sources\eMule0.72a-Sources\srchybrid`. En cas de doute sur un comportement filaire, lire le code C++ original (ex. `Packets.cpp`, `ClientUDPSocket.cpp`, `kademlia/`).

## Contraintes imposées par le code

- `#![forbid(unsafe_code)]` — aucun `unsafe` n'est possible, n'en ajoutez jamais.
- `#![deny(missing_docs)]` — tout item public doit être documenté.
- Zéro I/O, zéro réseau : cette crate est pure (données → octets, octets → données).

## Règles de modification

1. **Spec d'abord** : avant de toucher un opcode, un tag ou un cadrage, lire la spec correspondante dans `docs/ed2k/` (`opcodes.md`, `framing.md`, `message-catalog.md`, `messages/`) et les ADR dans `docs/ed2k/decisions/`.
2. **Limites bornées** : tout décodage doit respecter `limits.rs` (`MAX_FRAME_BYTES`, `MAX_STRING_BYTES`, `MAX_TAGS_PER_MESSAGE`, `PacketLimits`). Jamais d'allocation proportionnelle à un champ de longueur non validé.
3. **Déterminisme** : l'encodage doit être déterministe (round-trip `encode(decode(x)) == x` attendu). Les constantes filaires (`ED2K_BLOCK_SIZE`, `ED2K_PART_SIZE`, endianness little-endian) ne se changent pas sans vecteur de test de compatibilité.
4. **Erreurs** : utiliser `FramingError`/`ProtocolError` (`thiserror`) — jamais de `panic!`/`unwrap()` sur des données réseau.
5. **Feature `lab-loopback`** : le module `lab` est réservé aux tests loopback ; ne rien y ajouter qui fuiterait en production.
6. **API publique** : `lib.rs` ré-exporte largement (`pub use`); tout renommage de type public est un breaking change — vérifier les usages dans `mule-daemon` et `mule-mcp`.

## Tests attendus

- Tout nouveau message/opcode : test round-trip encoder/decoder + vecteur binaire connu si disponible dans `docs/ed2k/`.
- Toute limite : test de rejet au-delà de la borne (fuzzing `proptest` bienvenu, déjà en dev-dependency workspace).
- `cargo test -p mule-protocol --all-features` doit passer avant tout commit.

## Architecture interne

| Module | Rôle |
| :--- | :--- |
| `ed2k/` | Codecs eD2K : `decoder/`, `encoder/`, `opcodes.rs`, `tags.rs`, `session`, `source_exchange`, `obfuscation` |
| `kad/` | Messages DHT Kademlia (distance 128-bit `CUInt128`) |
| `framing/` | `PacketCodec` / `RawPacket` — cadrage header 5 octets |
| `limits.rs` | Bornes de sécurité et trait `PacketLimits` |
| `traits.rs` | Traits de scheduling découplés |
| `types.rs` | Types filaires partagés |
