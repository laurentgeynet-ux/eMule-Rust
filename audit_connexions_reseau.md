# Audit — Gestion des Connexions Réseau · Emule-Sion

> Portée : réseau **eD2K (TCP)**, réseau **Kad (UDP/DHT)**, **UPnP/WAN**
> Base de code : `d:\Projet\Emule-Sion` — crates `mule-daemon`, `mule-protocol`, `mule-core`, `mule-network-policy`

---

## Tableau de Bord des Risques

| Réseau | 🔴 Critiques | 🟠 Élevés | 🟡 Moyens | Score global |
|--------|:-----------:|:---------:|:---------:|:------------:|
| eD2K   | 3           | 3         | 6         | **Risque Élevé** |
| Kad    | 3           | 2         | 6         | **Risque Élevé** |
| UPnP   | 3           | 4         | 3         | **Risque Élevé** |

---

## 1. Réseau eD2K (TCP)

### 1.1 Points Forts

L'architecture de politique réseau (`mule-network-policy`) est **la pièce la mieux conçue** du projet.

| Mécanisme | Description |
|-----------|-------------|
| **7-layer policy** | `request_tcp_connect()` enchaîne : état cycle de vie → restriction protocole → filtre IP → token bucket → anti-SSRF → budgets → permit RAII |
| **Permits RAII** | `SocketPermit` / `ListenPermit` incrémentent/décrémentent atomiquement ; aucune fuite de compteur possible même en cas de panique |
| **Kill Switch atomique** | `disconnect_all()` : `ShuttingDown` → clone + cancel du `CancellationToken` → `Disabled`. Toutes les boucles select! l'écoutent |
| **Validator anti-SSRF** | `classify_ipv4()` couvre RFC 1918, RFC 6598 (CGNAT 100.64/10), RFC 3927, 240/4. `canonicalize_ip()` déplie les IPv4-mappées-IPv6 |
| **Timeouts par phase** | `PeerTimeouts` : 6 phases distinctes (`connect`, `handshake`, `status`, `queue`, `first_data`, `inactivity`) |
| **State machine formelle** | `Ed2kSession` — transitions illégales → état `Failed` irréversible |
| **Sequencer anti-doublon** | `BlockRequestTracker` rejette doublons et chevauchements ; `Ed2kBlockSink` rejette blocs déjà reçus |
| **Audit log privacy** | `redact_socket_addr()` masque les 2 derniers octets IPv4 publiques ; log circulaire ≤ 500 entrées |
| **Backoff exponentiel** | `min(600_000, 2^(fail-1) × 5_000)` ms, cap 10 min, persisté en SQLite |

---

### 1.2 Problèmes Identifiés

#### 🔴 ED-P1 — Signature SecIdent non vérifiée cryptographiquement
**Fichier :** [`inbound_listener.rs` L.282–287](file:///d:/Projet/Emule-Sion/crates/mule-daemon/src/services/inbound_listener.rs#L282-L287)

```rust
let is_valid = if let Ok(sig) = decoder.decode_signature(...) {
    !sig.signature_bytes.is_empty() && sig.signature_bytes.len() >= 16
} else { false };
```

La validation de `OP_SIGNATURE` se limite à vérifier que le buffer fait ≥ 16 octets. **Aucune vérification RSA/ECDSA.** Tout attaquant peut envoyer 16 octets aléatoires, obtenir `state: Authenticated` et le `credit_multiplier` associé.

**Fix :** Implémenter RSA PKCS#1 v1.5 sur la clé publique reçue via `OP_PUBLICKEY`, ou désactiver l'état `Authenticated` jusqu'à implémentation complète.

---

#### 🔴 ED-P2 — Identité factice dans `OP_HELLO_ANSWER`
**Fichier :** [`inbound_listener.rs` L.204–210](file:///d:/Projet/Emule-Sion/crates/mule-daemon/src/services/inbound_listener.rs#L204-L210)

```rust
let ans = Ed2kHelloAnswer {
    user_hash: [0x55u8; 16],   // hash statique — illégal dans eD2K
    client_id: 0x0100007F,     // 127.0.0.1 codé en dur
    tcp_port: remote_addr.port(), // port éphémère du PAIR, pas du LOCAL
    ...
};
```

Trois défauts : hash statique identique pour tous les clients inbound, `client_id` loopback incorrect, port local erroné. Impact : identification corrompue, crédits invalides, rejet potentiel par le serveur eD2K.

**Fix :** Injecter le vrai `user_hash`, `local_client_id` et `local_tcp_port` via une structure d'identité locale.

---

#### 🔴 ED-P3 — Double comptage incohérent des listeners
**Fichier :** [`manager.rs` L.395–398](file:///d:/Projet/Emule-Sion/crates/mule-network-policy/src/manager.rs#L395-L398) + [`permit.rs` L.127](file:///d:/Projet/Emule-Sion/crates/mule-network-policy/src/permit.rs#L127)

`request_tcp_connect(..., ConnectionPurpose::Listener)` vérifie `active_listeners` mais émet un `SocketPermit` (dont le `Drop` décrémente également `active_listeners`). Le chemin est correct mais **confusant et fragile** — un refactoring peut briser l'invariant silencieusement.

---

#### 🟠 ED-P4 — Pas de limite de connexions inbound par IP
**Fichier :** [`inbound_listener.rs` L.104–109](file:///d:/Projet/Emule-Sion/crates/mule-daemon/src/services/inbound_listener.rs#L104-L109)

```rust
Ok((socket, remote_addr)) => {
    tokio::spawn(async move {
        service_clone.handle_inbound_connection(socket, remote_addr, ...).await;
    });
}
```

Après le filtre IP, **aucune limite** sur le nombre de connexions simultanées par IP. Vecteur DoS : épuisement des fd, mémoire (`read_buf = 65 536 o` × N), threads Tokio.

**Fix :** `DashMap<IpAddr, AtomicUsize>` avec limite (ex. 10 conn/IP, 500 global) ; `Semaphore` tokio.

---

#### 🟠 ED-P5 — `read_buf` à croissance illimitée
**Fichiers :** [`server_session.rs` L.325/354](file:///d:/Projet/Emule-Sion/crates/mule-daemon/src/services/server_session.rs#L325), [`peer_session.rs` L.568](file:///d:/Projet/Emule-Sion/crates/mule-daemon/src/services/peer_session.rs#L568), [`inbound_listener.rs` L.172](file:///d:/Projet/Emule-Sion/crates/mule-daemon/src/services/inbound_listener.rs#L172)

`read_buf.extend_from_slice()` dans la boucle de lecture sans vérification de taille maximale. Un pair malveillant envoie des données continues sans compléter de paquets → OOM.

**Fix :** Après chaque `extend_from_slice`, vérifier `read_buf.len() > max_frame_size_bytes` → fermer la connexion.

---

#### 🟠 ED-P6 — Mutex `DownloadService` tenu pendant les I/O SQLite
**Fichier :** [`inbound_listener.rs` L.257–259, 422–423](file:///d:/Projet/Emule-Sion/crates/mule-daemon/src/services/inbound_listener.rs#L257-L259)

Le `Mutex<DownloadService>` est acquis **à l'intérieur de la boucle de traitement réseau** pour des appels SQLite potentiellement bloquants. Sous charge, toutes les connexions inbound bloquent simultanément sur le même mutex.

**Fix :** Architecture actor/message-passing ou `tokio::task::spawn_blocking` pour SQLite.

---

#### 🟡 ED-P7 — `guard_is_connecting()` : lock à chaque paquet
**Fichier :** [`server_session.rs` L.444](file:///d:/Projet/Emule-Sion/crates/mule-daemon/src/services/server_session.rs#L444)

Un `inner.lock().await` est acquis à chaque itération de la boucle principale pour vérifier si le handshake est expiré. Race condition potentielle + contention inutile.

**Fix :** `AtomicBool is_connected`.

---

#### 🟡 ED-P8 — Données synthétiques `0x77` servies et créditées
**Fichier :** [`inbound_listener.rs` L.401, 423](file:///d:/Projet/Emule-Sion/crates/mule-daemon/src/services/inbound_listener.rs#L401)

```rust
let payload = vec![0x77u8; length as usize];
// ...
let _ = dl_svc.record_credit_upload(ph, pk_hex, length as u64); // crédite du faux contenu
```

**Fix :** `#[cfg(not(feature = "real-file-serving"))]` pour bloquer la compilation en mode release.

---

#### 🟡 ED-P9 — `OP_QUEUERANKING` : encodage à vérifier
**Fichier :** [`inbound_listener.rs` L.430–435](file:///d:/Projet/Emule-Sion/crates/mule-daemon/src/services/inbound_listener.rs#L430-L435)

Le champ `length = 5` (1 opcode + 4 rank). À valider contre la spec eD2K pour s'assurer que le calcul inclut ou exclut correctement l'opcode dans le champ longueur.

---

#### 🟡 ED-P10 — `stop()` n'attend pas la fin du listener
**Fichier :** [`inbound_listener.rs` L.131–138](file:///d:/Projet/Emule-Sion/crates/mule-daemon/src/services/inbound_listener.rs#L131-L138)

`h.abort()` sans `h.await` → `restart()` immédiat peut obtenir `EADDRINUSE`.

**Fix :** `h.abort(); let _ = h.await;`

---

#### 🟡 ED-P11 — Race condition sur `is_running` dans `start()`
**Fichier :** [`inbound_listener.rs` L.68, 87](file:///d:/Projet/Emule-Sion/crates/mule-daemon/src/services/inbound_listener.rs#L68)

Check-then-store non atomique : deux appels concurrents peuvent tous deux passer le check et tenter de binder le même port.

**Fix :** `AtomicBool::compare_exchange(false, true, SeqCst, SeqCst)`.

---

#### 🟡 ED-P12 — Snapshot émis pendant le lock du SwarmInnerState
**Fichier :** [`peer_session.rs` L.527–532](file:///d:/Projet/Emule-Sion/crates/mule-daemon/src/services/peer_session.rs#L527-L532) (et nombreux autres points)

`emit_event()` appelé avec le lock tenu. **Fix :** cloner le snapshot avant de relâcher le lock, émettre hors du lock.

---

#### 🟡 ED-P13 — `Ed2kBlockSink::new()` alloue `file_size` octets en mémoire
**Fichier :** [`sequencer.rs` L.476](file:///d:/Projet/Emule-Sion/crates/mule-protocol/src/ed2k/sequencer.rs#L476)

```rust
buffer: vec![0u8; file_size as usize]
```

Pour un fichier de 4 Go contrôlé par un pair distant, allocation immédiate → DoS / OOM. À usage tests uniquement, mais l'API publique est dangereuse.

---

## 2. Réseau Kad (UDP/DHT)

### 2.1 Points Forts

| Mécanisme | Description |
|-----------|-------------|
| **Bind + nonblocking** | `std::net::UdpSocket` → `set_nonblocking(true)` → `tokio::net::UdpSocket` : procédure correcte |
| **Boucle recv avec cancellation** | `tokio::select!` sur `cancel_token.cancelled()` : arrêt propre, pas de fuite de tâche |
| **Stop() complet** | Annule la tâche listen, libère le socket, purge toutes les transactions in-flight via `oneshot::Sender::send(Err(...))` |
| **Validation magic byte** | Rejet immédiat si ≠ `0xE4` / `0xE5` |
| **Bornes codec** | `MAX_KAD_RES_CONTACTS = 20`, `MAX_KAD_SEARCH_SOURCES = 50`, `MAX_KAD_SEARCH_KEYWORDS = 50`, `MAX_KAD_UDP_PACKET_SIZE = 1472` |
| **Encodage borné** | `.min(MAX_KAD_RES_CONTACTS)` dans tous les `encode_*` |
| **Double filtre import** | Contacts bootstrap + `nodes.dat` → `classify_ip()` + `ip_filter.check_ip()` |
| **Timeout universel** | `KAD_TRANSACTION_TIMEOUT_SECS = 5` dans toutes les fonctions `send_*` |
| **TX IDs aléatoires** | `rand::random::<u32>() | 1` — difficilement prédictibles |
| **K-buckets K=20** | LRU correct, auto-exclusion du nœud local |

---

### 2.2 Problèmes Identifiés

#### 🔴 KAD-P1 — Absence totale de filtrage IP sur les paquets entrants
**Fichier :** [`kad_udp.rs` L.118–148](file:///d:/Projet/Emule-Sion/crates/mule-daemon/src/services/kad_udp.rs#L118-L148)

```rust
Ok((len, peer_addr)) => {
    // ← aucun classify_ip(peer_addr.ip()) ici
    match KadCodec::decode(packet_slice) {
        Ok(msg) => Self::handle_incoming_message(...)
    }
}
```

Toute IP (bogon, RFC1918, spoofée) peut envoyer un paquet et déclencher le handler. Contradictoire avec le filtrage appliqué dans `bootstrap_from_node`.

**Fix :**
```rust
let cat = classify_ip(peer_addr.ip());
let allowed = match profile { LocalLab => cat == Loopback, _ => cat == PublicRoutable };
if !allowed || ip_filter.check_ip(peer_addr.ip()).is_some() { continue; }
```

---

#### 🔴 KAD-P2 — Amplification UDP : réponses inconditionnelles à toute requête
**Fichier :** [`kad_udp.rs` L.706–825](file:///d:/Projet/Emule-Sion/crates/mule-daemon/src/services/kad_udp.rs#L706-L825)

```rust
KadMessage::Ping(_)        => { socket.send_to(&pong_bytes, peer_addr).await; }
KadMessage::HelloReq(_)    => { socket.send_to(&hello_res,  peer_addr).await; }
KadMessage::BootstrapReq(_)=> { socket.send_to(&bootstrap,  peer_addr).await; }
// Idem : SearchSourceReq, PublishSourceReq, SearchKeywordReq, PublishKeywordReq
```

**Aucun rate-limiting, aucune vérification de réciprocité.** Un attaquant spoofant l'IP d'une victime déclenche des réponses vers cette victime (DRDoS). Ratio ~1.05× par nœud mais multiplicateur × nombre de nœuds.

**Fix :** `HashMap<IpAddr, TokenBucket>` à 20 req/s par IP ; cookie challenge pour `HelloReq`.

---

#### 🔴 KAD-P3 — Matching de transaction par IP:port seule — injection de réponse forgée
**Fichier :** [`kad_udp.rs` L.712–724](file:///d:/Projet/Emule-Sion/crates/mule-daemon/src/services/kad_udp.rs#L712-L724)

```rust
for (id, tx) in inf.iter() {
    if tx.target == peer_addr && tx.expected_opcode == KADEMLIA2_PONG {
        matched_tx = Some(*id);
        break;
    }
}
```

Pas de nonce dans les paquets Kad2. Un attaquant capable d'injecter un paquet UDP avec la bonne IP:port source peut injecter une réponse forgée et faire matcher la transaction — notamment pour empoisonner la routing table via une `BootstrapRes` forgée.

---

#### 🟠 KAD-P4 — `in_flight` non borné — épuisement mémoire
**Fichier :** [`kad_udp.rs` L.43](file:///d:/Projet/Emule-Sion/crates/mule-daemon/src/services/kad_udp.rs#L43)

`HashMap<u32, InFlightTransaction>` sans limite. Un burst de requêtes parallèles (ex. publish vers 20 contacts) peut créer un pic mémoire. Le timeout de 5s assure le cleanup, mais pas le pic.

**Fix :** Check `if inf.len() >= MAX_IN_FLIGHT { return Err(...); }` avant insertion (max suggéré : 200).

---

#### 🟠 KAD-P5 — Contacts non-vérifiés insérés directement dans la routing table
**Fichier :** [`kad.rs` L.215–220](file:///d:/Projet/Emule-Sion/crates/mule-daemon/src/services/kad.rs#L215-L220), [`routing_table.rs` L.148–164](file:///d:/Projet/Emule-Sion/crates/mule-core/src/kad/routing_table.rs#L148-L164)

```rust
for c in &valid_contacts {
    table.insert_or_update(c.clone()); // verified: false, mais find_closest ne filtre pas
}
```

`find_closest` inclut les contacts `verified: false` dans les lookups XOR. Un nœud malveillant peut retourner 20 contacts forgés → **attaque Eclipse/Sybil**.

**Fix :** Ping de validation avant insertion, ou filtrer sur `verified == true` dans `find_closest` pour les lookups critiques. Limite de contacts par /24 par bucket.

---

#### 🟡 KAD-P6 — Opcode collision documentaire `KADEMLIA2_BOOTSTRAP_REQ == KADEMLIA2_REQ == 0x21`
**Fichier :** [`opcodes.rs` L.29–35](file:///d:/Projet/Emule-Sion/crates/mule-protocol/src/kad/opcodes.rs#L29-L35)

Intentionnel (protocole eMule) mais source de bug lors d'un futur refactoring. **Fix :** commentaire `// SAFETY: identiques par design du protocole Kad2 — ne jamais différencier`.

---

#### 🟡 KAD-P7 — Lookup non-itératif : un seul round de 3 requêtes
**Fichier :** [`kad.rs` L.231–258](file:///d:/Projet/Emule-Sion/crates/mule-daemon/src/services/kad.rs#L231-L258)

Kademlia standard requiert des rounds successifs jusqu'à convergence. Un seul round est effectué. Un attaquant contrôlant les 3 premiers contacts peut court-circuiter le lookup.

---

#### 🟡 KAD-P8 — `PublishSourceReq` entrant accepté sans validation de l'IP contact
**Fichier :** [`kad_udp.rs` L.784–787](file:///d:/Projet/Emule-Sion/crates/mule-daemon/src/services/kad_udp.rs#L784-L787)

Réponse `status=0` systématique. `req.contact.ip != peer_addr.ip()` n'est pas vérifié → acceptation de publications de proxy-IP.

---

#### 🟡 KAD-P9 — `ping_contact` : `verified: true` sans confirmation du Kad ID
**Fichier :** [`kad.rs` L.151–179](file:///d:/Projet/Emule-Sion/crates/mule-daemon/src/services/kad.rs#L151-L179)

Si `target_kad_id_hex` est `None`, un KadId **aléatoire** est généré, puis le contact est marqué `verified: true` sur simple Pong reçu. Un Pong ne prouve pas le Kad ID.

---

#### 🟡 KAD-P10 — `from_utf8_lossy` sur noms de fichiers reçus
**Fichier :** [`codec.rs` L.716, 762](file:///d:/Projet/Emule-Sion/crates/mule-protocol/src/kad/codec.rs#L716)

Les bytes invalides UTF-8 sont remplacés par `\u{FFFD}` silencieusement. Risque d'injection via homoglyphes ou null bytes.

---

#### 🟡 KAD-P11 — `name_len` (u16 jusqu'à 65535) non borné à la réception
**Fichier :** [`codec.rs` L.711–715](file:///d:/Projet/Emule-Sion/crates/mule-protocol/src/kad/codec.rs#L711-L715)

L'encodeur limite à 255, mais pas le décodeur. **Fix :** `let name_len = name_len.min(255);`

---

## 3. UPnP & Découverte WAN

### 3.1 Architecture UPnP

| Aspect | Valeur |
|--------|--------|
| Crate | `igd-next = "0.17"` (workspace `Cargo.toml` L.43) |
| API | **Synchrone** (`search_gateway`, `add_port`, `remove_port`, `get_external_ip`) |
| Feature `aio` | ❌ **Non activée** |
| Enveloppe async | `tokio::task::spawn_blocking` + `timeout(4s)` |
| TTL des mappings | **3600 s** (1h) codé en dur |
| Découverte | SSDP / IGD UPnP |
| STUN / NAT-PMP / PCP | ❌ Absents |

### 3.2 Flux de démarrage
```
main.rs:226 → if upnp_enabled && is_preview()
  → tokio::spawn(upnp_svc.map_ports(...))   // fire-and-forget (let _ = ...)
      → spawn_blocking { search_gateway(timeout=2s) }  // timeout externe 4s
      → cached_gateway stocké dans Arc<RwLock<Option<Gateway>>>
      → add_port TCP(3600s) + add_port UDP(3600s)
      → get_external_ip()
      ← snapshot mis à jour
      ← AUCUN REFRESH PÉRIODIQUE ← LACUNE CRITIQUE
Arrêt → unmap_ports() [timeout 500ms]
```

---

### 3.3 Problèmes Identifiés

#### 🔴 UPnP-L1 — Aucun renouvellement de bail — ports fermés après 1h
**Fichier :** [`upnp.rs`](file:///d:/Projet/Emule-Sion/crates/mule-daemon/src/services/upnp.rs) + [`main.rs` L.226–232](file:///d:/Projet/Emule-Sion/crates/mule-daemon/src/main.rs#L226-L232)

Le TTL est de 3600s mais aucune tâche périodique ne renouvelle le bail. Après 1h, les ports sont fermés par la box, rendant le nœud inaccessible depuis l'extérieur.

**Fix :**
```rust
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(45 * 60));
    interval.tick().await; // skip premier tick immédiat
    loop {
        interval.tick().await;
        let _ = upnp_svc.map_ports(ed2k_port, kad_port).await;
    }
});
```

---

#### 🔴 UPnP-L2 — `tcp_mapped`/`udp_mapped` = `true` même si `add_port` échoue
**Fichier :** [`upnp.rs` L.185–186](file:///d:/Projet/Emule-Sion/crates/mule-daemon/src/services/upnp.rs#L185-L186)

```rust
// Résultat add_port ignoré ; snapshot toujours mis à true dans Ok(...)
snap.tcp_mapped = true;
snap.udp_mapped = true;
```

L'utilisateur et l'UI croient les ports mappés alors que `add_port` a silencieusement échoué (port déjà occupé, etc.).

**Fix :**
```rust
let tcp_ok = gateway.add_port(...).is_ok();
let udp_ok = gateway.add_port(...).is_ok();
// puis dans Ok(...) :
snap.tcp_mapped = tcp_ok;
snap.udp_mapped = udp_ok;
```

---

#### 🔴 UPnP-L3 — Aucune validation de l'IP WAN retournée
**Fichier :** [`upnp.rs`](file:///d:/Projet/Emule-Sion/crates/mule-daemon/src/services/upnp.rs)

```rust
let ext_ip = gateway.get_external_ip()...?;
snap.external_ip = Some(ext_ip.to_string()); // jamais validé
```

Une gateway malveillante peut retourner une RFC1918, loopback, CGNAT ou IP tierce. L'utilisateur affiche alors une fausse IP publique.

**Fix :**
```rust
fn is_routable_public_ipv4(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            !v4.is_private() && !v4.is_loopback() && !v4.is_link_local()
            && !v4.is_broadcast() && !v4.is_multicast() && !v4.is_unspecified()
            && !(v4.octets()[0] == 100 && (v4.octets()[1] & 0xC0) == 64) // CGNAT
        }
        IpAddr::V6(_) => false,
    }
}
```

---

#### 🟠 UPnP-L4 — Aucun fallback si UPnP indisponible
**Fichier :** [`upnp.rs`](file:///d:/Projet/Emule-Sion/crates/mule-daemon/src/services/upnp.rs)

Si UPnP échoue, le daemon continue sans connectivité entrante active. Aucun fallback STUN, NAT-PMP ou saisie manuelle d'IP WAN n'est proposé. Aucun événement WebSocket n'alerte l'UI.

**Fix prioritaire :** Fallback STUN via `reqwest` (déjà présent en dépendance) ; sinon a minima un champ `wan_ip_manual` en configuration.

---

#### 🟠 UPnP-L5 — API synchrone dans `spawn_blocking` au lieu de la feature `aio`
**Fichier :** [`Cargo.toml` workspace L.43](file:///d:/Projet/Emule-Sion/Cargo.toml#L43)

`igd-next` possède une feature `aio` pour une API native async. L'usage de `spawn_blocking` avec l'API sync peut saturer le thread pool Tokio si plusieurs requêtes UPnP sont déclenchées simultanément.

**Fix :**
```toml
igd-next = { version = "0.17", features = ["aio"] }
```

---

#### 🟠 UPnP-L6 — Aucune protection contre un IGD malveillant (Rogue IGD)
SSDP répond en multicast UDP sur le LAN. N'importe quel hôte peut se déclarer gateway IGD. Un attaquant sur le même réseau peut retourner une fausse IP WAN et logger toutes les requêtes `add_port`.

**Fix partiel :** Vérifier que l'IP de la gateway SSDP découverte correspond à la gateway réseau du système.

---

#### 🟠 UPnP-L7 — Ports RPC non validés (port < 1024 accepté)
**Fichier :** [`rpc/methods.rs`](file:///d:/Projet/Emule-Sion/crates/mule-daemon/src/rpc) (handlers `upnp.mapPorts`)

`upnp.mapPorts` accepte n'importe quel `tcpPort`/`udpPort` sans vérifier `port >= 1024`. Un client authentifié peut mapper le port 80 ou 443.

**Fix :** `if tcp_port < 1024 || udp_port < 1024 { return Err(JsonRpcError::invalid_params(...)); }`

---

#### 🟡 UPnP-L8 — Aucun événement WebSocket lors d'un changement d'état UPnP
Les échecs, expirations, ou changements d'état UPnP ne produisent aucun event WebSocket. L'UI ne peut détecter un problème qu'en interrogeant périodiquement `upnp.status`.

---

#### 🟡 UPnP-L9 — IPv6 / IGDv2 / PCP non supportés
Seul IPv4 avec IGDv1 est supporté. `detect_local_ipv4_for_gateway()` retourne `None` sur IPv6.

---

#### 🟡 UPnP-L10 — Couverture de tests insuffisante
Les tests (`upnp_and_wan_tests.rs`) utilisent `with_upnp_enabled(false)`. Aucun test ne vérifie le comportement réel des mappings, la validation de l'IP WAN, ou le scénario de `add_port` échoué.

---

## 4. Matrice de Priorités (toutes couches)

| Ref | Réseau | Sévérité | Titre | Effort |
|-----|--------|----------|-------|--------|
| ED-P1 | eD2K | 🔴 Critique | Signature SecIdent non vérifiée | Moyen |
| ED-P2 | eD2K | 🔴 Critique | Identité fictive dans HELLO_ANSWER | Faible |
| KAD-P1 | Kad | 🔴 Critique | Pas de filtre IP sur boucle recv UDP | Faible |
| KAD-P2 | Kad | 🔴 Critique | Amplification UDP — réponses inconditionnelles | Moyen |
| KAD-P3 | Kad | 🔴 Critique | Transaction hijacking par injection UDP | Élevé |
| UPnP-L1 | UPnP | 🔴 Critique | Aucun refresh bail — ports fermés après 1h | Faible |
| UPnP-L2 | UPnP | 🔴 Critique | `tcp_mapped=true` même si `add_port` échoue | Faible |
| UPnP-L3 | UPnP | 🔴 Critique | IP WAN non validée (RFC1918, loopback…) | Faible |
| ED-P4 | eD2K | 🟠 Élevé | Pas de limite connexions inbound par IP (DoS) | Moyen |
| ED-P5 | eD2K | 🟠 Élevé | `read_buf` illimité (OOM) | Faible |
| ED-P6 | eD2K | 🟠 Élevé | Mutex DownloadService sur hot path réseau | Élevé |
| KAD-P4 | Kad | 🟠 Élevé | `in_flight` non borné — pic mémoire | Faible |
| KAD-P5 | Kad | 🟠 Élevé | Contacts non-vérifiés → Eclipse/Sybil | Moyen |
| UPnP-L4 | UPnP | 🟠 Élevé | Pas de fallback STUN/IP manuelle | Moyen |
| UPnP-L5 | UPnP | 🟠 Élevé | API sync dans spawn_blocking | Faible |
| UPnP-L6 | UPnP | 🟠 Élevé | Aucune protection contre Rogue IGD | Élevé |
| UPnP-L7 | UPnP | 🟠 Élevé | Ports RPC < 1024 acceptés | Faible |
| ED-P3 | eD2K | 🟡 Moyen | Double comptage listeners | Moyen |
| ED-P7 | eD2K | 🟡 Moyen | Lock par paquet pour guard handshake | Faible |
| ED-P8 | eD2K | 🟡 Moyen | Données synthétiques créditées | Faible |
| ED-P9 | eD2K | 🟡 Moyen | OP_QUEUERANKING framing à vérifier | Faible |
| ED-P10 | eD2K | 🟡 Moyen | stop() n'attend pas fin du listener | Faible |
| ED-P11 | eD2K | 🟡 Moyen | Race condition is_running start() | Faible |
| ED-P12 | eD2K | 🟡 Moyen | Snapshot émis sous lock | Faible |
| ED-P13 | eD2K | 🟡 Moyen | Ed2kBlockSink alloue file_size bytes | Faible |
| KAD-P6 | Kad | 🟡 Moyen | Opcode collision doc BOOTSTRAP_REQ==REQ | Faible |
| KAD-P7 | Kad | 🟡 Moyen | Lookup non-itératif (1 seul round) | Élevé |
| KAD-P8 | Kad | 🟡 Moyen | PublishSourceReq sans validation IP contact | Faible |
| KAD-P9 | Kad | 🟡 Moyen | verified:true sans confirmation Kad ID | Moyen |
| KAD-P10 | Kad | 🟡 Moyen | from_utf8_lossy silencieux | Faible |
| KAD-P11 | Kad | 🟡 Moyen | name_len non borné à la réception | Faible |
| UPnP-L8 | UPnP | 🟡 Mineur | Aucun événement WS changement UPnP | Faible |
| UPnP-L9 | UPnP | 🟡 Mineur | IPv6 / IGDv2 non supportés | Élevé |
| UPnP-L10 | UPnP | 🟡 Mineur | Couverture tests UPnP insuffisante | Moyen |

---

## 5. Synthèse Architecturale

### Ce qui est solide

> [!NOTE]
> La couche `mule-network-policy` est **exemplaire** : permits RAII, kill switch atomique, validator anti-SSRF complet, audit log privacy-preserving, profils restrictifs. La machine d'état `Ed2kSession` et le `Ed2kDownloadSequencer` sont également très bien conçus. La structure du socket UDP Kad avec annulation propre via `CancellationToken` est correcte.

### Ce qui est préoccupant

> [!CAUTION]
> Trois vecteurs de **DoS / amplification** sans rate-limiting : connexions TCP inbound illimitées (eD2K), réponses Kad UDP inconditionnelles, et absence de refresh UPnP rendant le nœud muet après 1h. Ces trois problèmes ont des corrections simples et doivent être traités avant toute mise en production.

> [!WARNING]
> La **couche applicative** (`inbound_listener.rs`) contient du code de prototype non sécurisé (hash fictif, signature SecIdent non vérifiée, données synthétiques `0x77` créditées). La routing table Kad accepte des contacts non-vérifiés dans `find_closest`, ouvrant la voie à une attaque Eclipse/Sybil.

### Bonnes nouvelles

> [!TIP]
> Tous les problèmes identifiés sont **correctables indépendamment** sans refactoring majeur. L'architecture globale est saine. Les 9 corrections 🔴 critiques et les 8 corrections 🟠 élevées représentent environ **3 sprints** de travail concentré.
