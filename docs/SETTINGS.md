# Paramètres Emule-Sion — Référence complète

Tous les paramètres sont persistés dans la table SQLite `settings` et
manipulables via RPC `settings.get` / `settings.set` (patch JSON en
camelCase). La majorité sont **appliqués à chaud** — pas de redémarrage du
daemon requis. La colonne « eMule » indique l'équivalent dans les
Préférences d'eMule 0.72a.

## Interface (`/settings`)

| Section UI | Contenu |
|---|---|
| **Général** | Pseudo, dossier de téléchargement, partage Incoming, rétention historique, langue |
| **Connexion** | Kill switch, profil réseau, quotas de sockets, **capacités & ports**, obfuscation, connexions max, USS, UPnP |
| **Transferts** | Sources max globales / par fichier / min, file d'attente upload, quota journalier, sauvetage de swarm |
| **Sécurité** | Filtre IP (`ipfilter.dat`), limites de débit, notice anti-SSRF, options exclusives (paranoid, rotation d'identité) |
| **Amis** | Friend slots (`friends.*`) |
| **Avancé** | Diagnostic, import `.part.met`, guide lab, barre debug, version |

## Clés RPC

### Général

| Clé `settings.set` | Clé persistée | Type | Défaut | Effet | eMule |
|---|---|---|---|---|---|
| `nickname` | `nickname` | string 1–64 | `"Emule-Sion"` | Annoncé au login serveur et dans les hellos pairs | `nick` |
| `defaultDownloadDir` | `default_download_directory` | path | dossier portable | Probe d'écriture validé ; utilisé à l'ajout de liens et au scan Incoming | « Répertoire » |
| `shareDownloadDir` | `share_download_dir` | bool | `true` | Scan+hash des fichiers du dossier Incoming (démarrage + `shares.rescan`) | dossier Incoming partagé |
| `historyRetentionDays` | `history_retention_days` | u32 | `30` | Purge de l'historique appliquée immédiatement | — |
| `tempDir` | `temp_dir` | path | dossier daemon | Dossier `.part` effectif pour les nouveaux downloads et imports | « Répertoire temporaire » |
| `ichEnabled` | `ich_enabled` | bool | `true` | Vérification MD4 de chaque part complétée (désactiver saute la vérification intermédiaire) | `ICH` |
| `rememberCancelled` | `remember_cancelled` | bool | `true` | Entrée d'historique à l'annulation | `RememberCancelledFiles` |
| `rememberDownloaded` | `remember_downloaded` | bool | `true` | Entrée d'historique à la complétion | `RememberDownloadedFiles` |

### Serveurs (section Connexion)

| Clé | Persistée | Type | Défaut | Effet | eMule |
|---|---|---|---|---|---|
| `serverReconnect` | `server_reconnect` | bool | `true` | Watcher 15 s : reconnexion au dernier serveur via le chemin RPC normal (policy + kill switch appliqués) | `Reconnect` |
| `serverAutoconnect` | `server_autoconnect` | bool | `false` | Au premier tick, connexion au serveur de meilleure `preference` non désactivé | `Autoconnect` |
| `serverAutoremoveDead` | `server_autoremove_dead` | bool | `true` | Supprime la cible après 1 échec de reconnexion (eMule `DeadServerRetry` défaut = 1) | `AutoDeadServer`/`DeadServerRetry` |
| `serverAutoupdateList` | `server_autoupdate_list` | bool | `false` | Importe `server_autoupdate_path` une fois par session | `AutoUpdateServerList` |
| `serverAutoupdatePath` | `server_autoupdate_path` | path ≤512 | `""` | Fichier `server.met` **local** (aucun fetch HTTP par design anti-SSRF) | URL server.met |
| `safeConnect` | `safe_connect` | bool | `false` | `downloads.resume` refusé tant que ni serveur eD2K ni Kad (contacts > 0) ne sont disponibles | `SafeConnect` |

### Connexion

| Clé | Persistée | Type | Défaut | Effet | eMule |
|---|---|---|---|---|---|
| `ed2kPort` | `ed2k_port` | u16 1–65535 | `39735` | **Rebind à chaud** du listener TCP + `kad.set_ed2k_tcp_port` + buddy connect-options + `upnp.set_ports` | Port TCP (eMule randomise 4096–65095 au 1er démarrage) |
| `kadPort` | `kad_port` | u16 1–65535 | `39737` | Rebind à chaud de la socket Kad UDP | Port UDP (idem, randomisé par eMule) |
| `udpEnabled` | `udp_enabled` | bool | `true` | Stoppe/démarre `kad_udp` + `server_udp` | « UDP désactivé » |
| `upnpEnabled` | `upnp_enabled` | bool | `false` | `map_ports`/`unmap_ports` immédiat | `EnableUPnP` (défaut eMule : off) |
| `capacityDownloadKbs` | `capacity_download_kbs` | Ko/s | `0` | Capacité de ligne descendante — borne la limite DL à 90 % de la capacité (clamp eMule `Preferences.cpp`), réappliqué à chaque `limits.set` et normalisé au boot | Capacité Réception |
| `capacityUploadKbs` | `capacity_upload_kbs` | Ko/s | `0` | Capacité montante — borne la limite UL à 80 % de la capacité ; référence USS (`note_manual_limit`) | Capacité Émission |
| `obfuscationEnabled` | `obfuscation_enabled` | bool | config | Listener inbound live (`AtomicBool`), connexions sortantes, byte buddy Kad | « Support obfuscated connections » |
| `obfuscationRequired` | `obfuscation_required` | bool | config | idem — pairs sans chiffrement rejetés | « Require obfuscated connections » |
| `maxConnections` | `max_connections` | usize | `500` | `ResourceBudgets.max_active_peer_connections` (live) | MaxConnections |
| `maxConnPerMin` | `max_conn_per_min` | u32 | `240` | `ResourceBudgets.rate_limit_per_min` (live) | MaxConnPerFiveSeconds = 20 ≈ 240/min (granularité min) |
| `ussEnabled` | `uss_config` (JSON) | bool | `false` | `UssService.update_config` — limite d'upload pilotée par latence | « USS » |
| `paranoidMode` | `paranoid_mode` | bool | `false` | Slots d'upload réservés aux pairs **obfusqués + signature SUI vérifiée** ; rejette les clients eMule classiques (live, `dispatcher`) | — (spécifique Emule-Sion) |
| `identityRotationHours` | `identity_rotation_hours` | h 0–8760 | `0` | Régénère périodiquement clé RSA + user_hash eD2K + KadID (propagation à chaud listener/Kad ; sessions actives conservent l'ancienne). **Invalide les crédits** — identité éphémère. Timestamp dans `identity_last_rotation_unix` | — (spécifique Emule-Sion) |

### Transferts

| Clé | Persistée | Type | Défaut | Effet | eMule |
|---|---|---|---|---|---|
| `maxGlobalDownloadPeers` | `max_global_download_peers` | usize | `500` | Quota global du swarm + budget sockets | Sources max |
| `maxPeersPerDownload` | `max_peers_per_download` | usize | `400` | Plafond de sources par fichier (surchargeable via `downloads.setMaxSources` / `FT_MAXSOURCES`) | `MaxSourcesPerFile` |
| `minPeersPerDownload` | `min_peers_per_download` | usize | `5` | Quota garanti sous saturation | — |
| `maxUploadQueue` | `max_upload_queue` | usize 10–5000 | `5000` | `UploadManager.max_queue` (live) | `QueueSize` (défaut eMule 5000) |
| `minUploadKbs` | `min_upload_kbs` | Ko/s 0–100000 | `1` | Plancher par slot : pas de nouveau slot si la moyenne descend sous le seuil (live) | `MinUpload` (défaut eMule 1) |
| `autoPriorityNewDownloads` | `auto_priority_new_downloads` | bool | `false` | Fichiers ≥ 9 728 000 B créés en priorité `high` | auto-priorité |
| `dailyUploadQuotaGb` | `daily_upload_quota_gb` | Gio 0–1048576 | `0` | Quota volumétrique/jour : atteint → plus aucune nouvelle demande de slot accordée (watcher 30 s). Compteur persisté `upload_quota_used_bytes`, remise à zéro au changement de jour UTC (`upload_quota_day`) | — (spécifique Emule-Sion, aucun équivalent eMule) |
| `swarmHealing` | `swarm_healing` | bool | `false` | Biaise la sélection de part vers les **parts les plus rares du swarm** (compteur de sources connues par part) — le client devient source de secours avant la disparition d'une part. Tie-break : octets reçus | — (spécifique Emule-Sion ; eD2K est pull-only, aucun push possible) |

### Limites de débit (RPC `limits.set`, section Sécurité)

Throttles DL/UL en octets/s appliqués aux buckets globaux ; distincts des
*capacités* (vitesse de ligne, utilisées par USS comme référence). Chaque
champ fourni est **persisté** (`rate_limit_download_bps`,
`rate_limit_upload_bps`, `rate_limit_conn_per_min`,
`rate_limit_server_req_per_sec`, `rate_limit_kad_pkt_per_sec`) et rechargé
au démarrage dans `AppState::new`. Une mise à jour **partielle** conserve
les autres limiteurs (la config part du snapshot live des buckets, pas des
défauts compilés).

**Clamp capacité** (sémantique eMule `Preferences.cpp`) : les limites DL/UL
sont bornées à **90 %** / **80 %** des capacités déclarées — `0` (illimité)
n'est jamais borné, idem si la capacité est `0`. Le clamp s'applique à
`limits.set`, à tout changement de `capacity*Kbs` (`apply_capacity_clamp`,
la valeur effective est repersistée), et **au démarrage** : une limite
persistée au-delà de la capacité est bornée puis réécrite en SQLite. L'UI
affiche la valeur réellement appliquée (relecture de `limits.get`) et le
signale dans l'onglet des débits.

### Filtre IP (RPC `filter.*`, section Sécurité)

| RPC | Persistée | Effet |
|---|---|---|
| `filter.setEnabled` | `ipfilter_enabled` (bool) | Active/désactive le filtrage entrant+sortant ; restauré au boot |
| `filter.importFile` | `ipfilter_path` (path) | Charge un `ipfilter.dat` **local** (pas de fetch HTTP, anti-SSRF) ; le fichier est rechargé automatiquement au démarrage |
| `filter.clear` | `ipfilter_path` → `""` | Vide les ranges et oublie le chemin persisté |

Comme eMule qui recharge `ipfilter.dat` à chaque démarrage, le filtre et
son état activé survivent aux redémarrages du daemon.

## Comportements notables

- **Rebind TCP** : changer `ed2kPort` coupe brièvement le listener (comme
  eMule). Les connexions entrantes actives sont interrompues.
- **UDP off** : le client-UDP partage la socket Kad — désactiver UDP coupe
  les trois services d'un coup (Kad UDP, server UDP, client UDP).
- **Partage partiel** : les téléchargements en cours apparaissent dans
  `shares.list` avec `isPartial=true` dès qu'une part de 9 728 000 B est
  complète ; seules les parts complétées sont servies (`PARTSTATUS`).
  Les partiels sont publiés comme **sources Kad** (`PublishSrc`) mais
  exclus de la publication de keywords ; dans l'`OP_OFFERFILES` serveur
  (trié par priorité d'upload, ≤ 200 entrées) ils portent les marqueurs
  `0xFCFCFCFC`/`0xFCFC`. `shares.setEnabled`/`setUploadPriority` les
  rejettent — un partage transitoire persisté figerait le chemin `.part`
  et le servirait comme complet après restart.
- **Partages après restart** : les downloads `Completed` sont lus depuis
  SQLite même hors de la map `active` — la liste de partage survit aux
  redémarrages (le fichier cible doit exister sur disque).
- **`shares.rescan`** : rescane le dossier Incoming (bouton refresh de
  l'onglet Partages) ; retourne `{added, dir}` ; respecte `shareDownloadDir`.
- **Boot** : tous les overrides persistés sont relus dans `AppState::new`
  avant le bind des sockets — les valeurs survivent aux redémarrages.
- **Watcher serveur** (`AppState::spawn_server_watcher`, tick 15 s) :
  ordre = reconnect (`last_target`) → autoconnect (meilleur serveur) ;
  skip si kill switch actif ; 1 échec → `delete_server` si
  `server_autoremove_dead`. La déconnexion manuelle efface `last_target`.
- **ICH off** : les parts complétées ne sont plus vérifiées par MD4 — la
  complétion repose sur la couverture des ranges et le hash global final.
  AICH recovery n'est pas impacté (il dépend de `aich_root`).

## UX de la page `/settings`

- **Sauvegarde** : les switches s'appliquent immédiatement ; les champs
  texte/numériques se soumettent par Entrée ou bouton « Appliquer »/icône.
- **Validation** : toute saisie invalide (hors bornes, vide) remonte une
  erreur localisée dans la bannière globale — plus de toast « mis à
  jour » quand rien n'a été appliqué.
- **Synchronisation multi-clients** : `settings.changed` resynchronise
  switches **et** champs texte ; un champ en cours d'édition (dirty)
  n'est jamais écrasé par une valeur distante.
- **Kill switch** : confirmé par un dialog avant `network.disconnectAll`.
- **Rétention** : une valeur persistée hors items standards {7, 30, 90, 0}
  (ex. via MCP) s'affiche dans un item dédié du dropdown.
- **Localisation** : la totalité de la page (sections, cartes réseau,
  messages de statut) est couverte par les ARB FR/EN.
