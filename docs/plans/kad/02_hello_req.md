# Étape 2 — `HELLO_REQ` sortant

> **Dépendances** : étape 1 (le `ContactVerifier` consomme la primitive).
> **Risque interop** : nul — HELLO est le mécanisme de vérification standard Kad ≥ 8 ; la chaîne ACK à 3 voies est déjà gérée côté entrant.

## Objectif

`KadCodec::encode_hello_req` existe (`encoder.rs:124-163`, `codec.rs:42-67`) mais **aucun appelant** : les contacts appris ne sont jamais vérifiés activement que par `ping` (qui prouve l'IP, pas l'identité Kad). HELLO vérifie KadID + version + ports — c'est le canal eMule pour « vérifier activement les nouveaux candidats utiles ».

## Changements

### 2a. `crates/mule-daemon/src/services/kad_udp/routing_client.rs`

`pub async fn send_hello(&self, target: SocketAddr, peer_version: u8) -> Result<KadHelloRes, String>` — même gabarit que `ping` (l.20-88) :

1. Transaction `KadExpectedResponse::HelloRes` (variante existante, `types.rs:58`).
2. `KadCodec::encode_hello_req(&local_id, udp_port, OUR_KAD_VERSION, tcp_port, misc_options)`.
3. À réception de `HelloRes` → envoyer `encode_hello_res_ack(&local_id)` (3-way, `KademliaUDPListener.cpp` : HELLO_RES attend un ACK qui installe la réciprocité).
4. Retourner la struct décodée (ports/ID vus par le pair).

### 2b. `contact_verifier.rs` — politique de sonde

| Cas | Primitive |
|---|---|
| Contact **non vérifié**, `version >= MIN_KAD2_HELLO_VERSION` | `send_hello` (prouve IP + identité + version) |
| Contact **vérifié** (LRU de bucket plein) ou version < seuil | `ping` (existant) |

**⚠️ À vérifier avant implémentation** : borne de version exacte dans `KademliaUDPListener.cpp` / `Kademlia.cpp` — eMule n'envoie HELLO qu'aux contacts Kad2 (`KADEMLIA_VERSION` ≥ seuil à confirmer dans `Protocol.h`/opcodes). Mettre la constante `MIN_KAD2_HELLO_VERSION` en tête de `contact_verifier.rs`.

### 2c. `handlers.rs`

Vérifier que `HelloRes` entrant (hors transaction attendue) et `HelloResAck` entrant alimentent `learn_contact` avec `mark_seen` — comportement déjà présent à confirmer.

## Tests

- Loopback : nœud factice répond `HelloRes` → `send_hello` retourne la struct + ACK émis (capturer le datagramme sortant, opcode `0x22`).
- Verifier : contact non vérifié Kad ≥ seuil → HELLO ; version inférieure → ping.

## Critères de fin

- [ ] `HELLO_REQ` émis et séquence `REQ → RES → ACK` observée en loopback.
- [ ] Un contact non vérifié promu `verified` après HELLO réussi.
- [ ] Aucun HELLO vers des pairs Kad1 (version < seuil).

## Référence eMule

`KademliaUDPListener.cpp` `Process_KADEMLIA2_HELLO_REQ/RES/RES_ACK` ; `CRoutingZone` utilise HELLO pour vérifier un contact avant insertion définitive.
