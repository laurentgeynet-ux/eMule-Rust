# Étape 8 — Buddy client LowID

> **Dépendances** : étapes 1 + 5 — exige des contacts **vérifiés** stables et un matching de transactions durci. C'est le morceau le plus lourd : le placer en dernier.
> **Risque interop** : `FINDBUDDY_REQ`/`CALLBACK_REQ` sont filaire-standard eMule — compatible par construction, **mais** vérifier le layout exact dans `KademliaUDPListener.cpp` + `Packets.cpp` avant d'encoder.

## Objectif

Aujourd'hui seul le rôle **relais** existe (`BuddyRegistry`, `types.rs:251-346` : pending/active, `BUDDY_PENDING_TTL`). Un nœud **LowID** ne peut pas publier ses sources ni être rappelé — il lui faut trouver et entretenir un buddy.

## État vérifié

- `BuddyRegistry` : `register_pending`, `take_pending`, `set_active`, `clear_active`, `send_to_buddy` — côté **relais** seulement.
- `FINDBUDDY_RES` / `CALLBACK_REQ` : décodage existant (à confirmer dans `decoder/`).
- Détection LowID : déjà connue côté eD2K (HighID vs LowID) — utilisée pour la publication de source.
- `persist_indexed_source` / publish path : `KadIndexedSource` porte déjà `buddy_id` (nullable) ? À vérifier — sinon ajouter le champ.

## Changements

### 8a. `mule-protocol` — `encode_findbuddy_req`

`encoder.rs` + `types.rs` wire : `FINDBUDDY_REQ` (0x51) — layout à vérifier dans `KademliaUDPListener.cpp` (`Process_KADEMLIA_FINDBUDDY_REQ`). Round-trip test + vecteur si disponible.

### 8b. `kad_udp/buddy_client.rs` *(nouveau fichier)*

```rust
pub struct BuddyClientState {
    pub our_buddy: Option<OurBuddy>,
}

pub struct OurBuddy {
    pub buddy_id: [u8; 16],
    pub buddy_addr: SocketAddr,       // pair relais
    pub link_tx: mpsc::UnboundedSender<Vec<u8>>, // notre socket TCP vers lui
    pub expires: Instant,
}
```

- `send_findbuddy(target, buddy_id) -> Result<FindBuddyRes, KadTxError>` — transaction sur `FINDBUDDY_REQ`.
- Sélection candidat : `verified && version >= MIN_BUDDY_VERSION && UDP-open` — les critères exacts sont dans eMule (`CClientList`/`Kademlia`) — à vérifier.

### 8c. `kad.rs` — établissement du lien

1. `is_low_id()` (depuis l'état eD2K) → si LowID, chercher un candidat.
2. `send_findbuddy` → `FINDBUDDY_RES` OK → **ouvrir une connexion TCP sortante** vers le buddy (peer_session dédiée, marquée « buddy link »).
3. Lien maintenu : TTL, un seul buddy à la fois (miroir `BuddyRegistry`), failover sur perte (clear → re-candidat).

### 8d. Publication LowID (`search_sources.rs` / publish path)

Publier une source LowID (`tcp_port=0` + `buddy_id` renseigné) **uniquement si `our_buddy` actif** — jamais sur la seule foi d'un `buddy_id` reçu. Le `CALLBACK_REQ` entrant doit être relayable par le buddy via le lien TCP.

## Tests

- Loopback : nœud LowID factice → `send_findbuddy` → RES → lien TCP ouvert → publication avec `buddy_id`.
- Failover : kill du lien → re-sélection d'un autre candidat.
- Jamais de publish LowID sans `our_buddy` actif.

## Critères de fin

- [ ] Source LowID publiée avec `buddy_id` valide et joignable via callback relayé.
- [ ] Un seul buddy ; failover sur perte du lien.
- [ ] Zéro publish LowID orphelin.

## Référence eMule

`KademliaUDPListener.cpp` `Process_KADEMLIA_FINDBUDDY_REQ/CALLBACK_REQ` ; `CClientList::m_pBuddy` ; `OP_REASKCALLBACKUDP` → `OP_REASKCALLBACKTCP`.
