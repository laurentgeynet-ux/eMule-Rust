# Index des fichiers du projet

Index exhaustif des fichiers versionnés du dépôt **Emule-Sion** — un client eDonkey/eMule écrit en Rust (daemon + crates protocolaires) avec une application de bureau Flutter.

---

## Racine

| Fichier | Fonction |
|---|---|
| `.gitignore` | Exclusions Git (target/, dist/, scratch/, etc.) |
| `Cargo.toml` | Manifeste du workspace Rust (membres, dépendances partagées) |
| `Cargo.lock` | Verrouillage des versions des dépendances Rust |
| `README.md` | Présentation générale du projet et portail vers docs/ |
| `AGENTS.md` | Règles pour agents IA (cartographie, validation, règles critiques) |
| `CLAUDE.md` | Lien physique vers `AGENTS.md` (compatibilité Claude Code) |
| `GEMINI.md` | Lien physique vers `AGENTS.md` (compatibilité Gemini CLI) |
| `.github/copilot-instructions.md` | Lien physique vers `AGENTS.md` (compatibilité GitHub Copilot) |
| `deny.toml` | Configuration cargo-deny (audit des licences/vulnérabilités) |
| `search_mgr.cpp` | Fichier C++ de référence (code eMule original, gestionnaire de recherche) |

## .agents/ — Configuration agents/MCP

| Fichier | Fonction |
|---|---|
| `.agents/mcp_config.json` | Configuration du serveur MCP |
| `.agents/plugins/emule-sion/mcp_config.json` | Configuration MCP du plugin emule-sion |
| `.agents/plugins/emule-sion/plugin.json` | Manifeste du plugin agent |

## apps/desktop/ — Application Flutter (Windows)

### Cœur de l'application (`lib/`)

| Fichier | Fonction |
|---|---|
| `lib/main.dart` | Point d'entrée : compose `app_shell` + `mule_client` |
| `lib/core/deep_link_handler.dart` | Liens `ed2k://` entrants |
| `lib/core/download_notifier.dart` | Notifications système sur événements de téléchargement |
| `lib/core/tray_setup.dart` | Icône et menu de la zone de notification |
| `lib/core/window_setup.dart` | Configuration de la fenêtre native |
| `lib/services/daemon_supervisor.dart` | Supervision du cycle de vie du daemon (démarrage/arrêt) |
| `lib/services/work_directory_resolver.dart` | Résolution du répertoire de travail du daemon |

### Configuration Flutter / plateformes

| Fichier | Fonction |
|---|---|
| `pubspec.yaml` | Manifeste du package Flutter (dépendances) |
| `pubspec.lock` | Verrouillage des dépendances Dart/Flutter |
| `analysis_options.yaml` | Règles d'analyse statique Dart |
| `.metadata` | Métadonnées du projet Flutter |
| `README.md` | Documentation de l'app desktop |
| `assets/app_icon.ico` | Icône de l'application |
| `android/`, `ios/`, `linux/`, `macos/`, `windows/` | Runners natifs générés par Flutter (seul `windows/` est la cible supportée : `windows/ed2k_protocol.reg` enregistre le protocole `ed2k://`, `runner/` contient le code Win32) |

### Tests Flutter (`test/`)

| Fichier | Fonction |
|---|---|
| `test/domain/download_item_test.dart` | Tests du modèle DownloadItem |
| `test/integration/live_daemon_e2e_test.dart` | Test E2E contre un daemon réel |
| `test/services/daemon_supervisor_test.dart` | Tests du superviseur de daemon |
| `test/services/work_directory_resolver_test.dart` | Tests du résolveur de répertoire de travail |
| `test/ui/*.dart` (17 fichiers) | Tests UI : catégories, diagnostics, détails, écrans downloads/history/Kad/search/servers, import part.met, IP filter, politique réseau, sources de pairs, responsive, performance grandes listes, settings |

## apps/web/ — Application Flutter Web

| Fichier | Fonction |
|---|---|
| `lib/main.dart` | Point d'entrée de l'app web |
| `lib/daemon_connect_screen.dart` | Écran de connexion au daemon |
| `pubspec.yaml` / `pubspec.lock` / `analysis_options.yaml` | Configuration Flutter |
| `web/` | Boilerplate web (index.html, manifest, icônes) |

## packages/app_shell/ — Shell UI partagé (MVVM)

Package Dart contenant toute l'interface : composition applicative (`lib/app/`), services transverses (`lib/core/`), localisation (`lib/l10n/`) et écrans par feature (`lib/ui/features/`).

### Configuration et composition

| Fichier | Fonction |
|---|---|
| `pubspec.yaml` / `analysis_options.yaml` / `l10n.yaml` | Configuration du package (dont génération i18n) |
| `lib/emule_sion_app.dart` | Widget racine de l'application |
| `lib/app/app_controller.dart` | Racine de composition : session daemon + wiring des contrôleurs |
| `lib/app/providers.dart` | Providers Riverpod de l'application |
| `lib/app/router.dart` | Routage/navigation entre écrans |
| `lib/app/shell_controller.dart` | Contrôleur du shell (état global UI) |
| `lib/app/locale_controller.dart` | Contrôleur de langue (FR/EN) |
| `lib/app/stats_controller.dart` | Contrôleur des statistiques globales |
| `lib/app/transfer_rate_tracker.dart` | Suivi des débits de transfert |

### Services transverses (`lib/core/`)

| Fichier | Fonction |
|---|---|
| `lib/core/daemon_lifecycle.dart` | Cycle de vie de la connexion daemon |
| `lib/core/folder_picker.dart` (+`_io`, `_stub`) | Sélecteur de dossier (abstraction + impl natif/stub) |
| `lib/core/system_actions.dart` (+`_io`, `_stub`) | Actions système (ouvrir dossier, révéler fichier…) |
| `lib/core/work_dirs.dart` | Répertoires de travail (downloads, incoming…) |

### Localisation (`lib/l10n/`)

| Fichier | Fonction |
|---|---|
| `lib/l10n/l10n.dart` | Point d'accès aux localisations |
| `lib/l10n/app_fr.arb` / `app_en.arb` | Ressources de traduction FR/EN |
| `lib/l10n/generated/` | Localisations générées (`app_localizations*.dart`) |

### Feature `downloads` (`lib/ui/features/downloads/`)

| Fichier | Fonction |
|---|---|
| `view_models/downloads_store_controller.dart` | Store principal des téléchargements |
| `view_models/categories_controller.dart` | Contrôleur des catégories |
| `view_models/lab_controller.dart` | Contrôleur du lab |
| `view_models/network_policy_controller.dart` | Contrôleur de la politique réseau |
| `view_models/peer_sources_controller.dart` | Contrôleur des sources de pairs |
| `views/downloads_screen.dart` | Écran principal des téléchargements |
| `widgets/downloads_list.dart`, `downloads_top_bar.dart`, `downloads_sidebar.dart`, `downloads_empty_state.dart` | Liste, barre d'outils, sidebar et état vide |
| `widgets/download_row.dart`, `download_details_panel.dart`, `download_detail_dialog.dart` | Ligne de download et panneau/dialogue de détail |
| `widgets/download_sources_expansion.dart` | Expansion des sources d'un download |
| `widgets/speed_sparkline.dart`, `status_notification_banner.dart`, `debug_simulation_bar.dart` | Sparkline de débit, bannière de statut, barre de simulation |
| `widgets/add_link_dialog.dart` + `widgets/add_link/` | Dialogue d'ajout de lien ed2k (`ed2k_link_parser`, dropdowns catégorie/allocation, preview) |
| `widgets/category_edit_dialog.dart` + `widgets/category/` | Édition de catégorie (color picker, dropdown priorité) |
| `widgets/detail/` (16 fichiers) | Cartes du détail : header, action bar, identité, métriques, part map, chemins, sources, diagnostics, dialogues destination/suppression, `peer_source_row` |
| `widgets/diagnostic_dialog.dart` + `widgets/diagnostic/` (8 fichiers) | Carte de diagnostic : chips de capacités, lignes d'info, formatage, bannière profil réseau |
| `widgets/import/` (7 fichiers) + `import_part_met_dialog.dart` | Import défensif `.part.met` : inspection, rapport, notice de sécurité |
| `widgets/lab/` (9 fichiers) + `lab_guide_dialog.dart` | Assistant lab : étapes fixture/lien/pair/connexion/nettoyage |
| `widgets/network/` (9 fichiers) + `network_policy_dialog.dart`, `ip_filter_dialog.dart` | Onglets IP filter et rate limits, quotas, profil réseau, audit log |
| `widgets/row/` (8 fichiers) | Ligne de download : variantes compacte/complète, badges, icônes, barre d'actions, pill de statut, chip de priorité (`download_priority_chip.dart`) |
| `widgets/settings/` (5 fichiers) | Briques réutilisées par la page Paramètres : dossier, langue, rétention, amis, message inline |
| `widgets/sidebar/` (9 fichiers) | Sidebar : header brand, items nav/catégorie/filtre, carte de statut, suppression de catégorie |
| `widgets/incompatible_version_dialog.dart` | Avertissement de version daemon incompatible |

### Features `history`, `kad`, `search`, `servers`, `settings`, `shares`, `uploads`

| Fichier | Fonction |
|---|---|
| `history/view_models/history_controller.dart` + `views/history_view.dart` + `widgets/` (9 fichiers) | Historique : vue, toolbar, filtre, stats dashboard, lignes d'entrée, dialogues détail/export/effacement |
| `kad/view_models/kad_controller.dart` + `views/kad_screen.dart` + `widgets/` (12 fichiers) | Kad : table de contacts, bootstrap (dialogue + URL), import nodes.dat, badges/statuts, bannière sécurité |
| `search/view_models/search_controller.dart` + `views/search_screen.dart` + `widgets/` (7 fichiers) | Recherche : formulaire, table de résultats, bannière connexion, états |
| `servers/view_models/servers_controller.dart` + `views/servers_screen.dart` + `widgets/` (12 fichiers) | Serveurs : table et cartes, import server.met, session bar, badges, dialogues clear/update |
| `settings/view_models/settings_controller.dart` + `views/settings_view.dart` + `views/sections/` (6 fichiers) + `widgets/` (4 fichiers) | Page Paramètres (`/settings`) : rail Général/Connexion/Transferts/Sécurité/Amis/Avancé ; pseudo, partage Incoming, profil/kill switch/quotas, **capacités & ports TCP/UDP, toggles UDP+UPnP**, obfuscation, connexions max, USS, limites de sources, file d'upload, IP filter, amis, outils avancés |
| `shares/views/shares_view.dart` | Onglet « Partages » (`/shares`) : fichiers servis (persistés + partiels `.part` badge « partiel »), priorité d'upload (powershare), switch activé/suspendu, **bouton rescan Incoming** (`shares.rescan`), rafraîchissement live sur events lifecycle/progression |
| `uploads/view_models/uploads_controller.dart` + `views/uploads_view.dart` + `widgets/uploads_table.dart` | Uploads : vue et table des envois |

## packages/mule_client/ — Client daemon partagé

| Fichier | Fonction |
|---|---|
| `lib/mule_client.dart` | Barrel : export unique du package |
| `lib/data/rpc_daemon_client.dart` | Client RPC réel communiquant avec le daemon (WebSocket/JSON-RPC) |
| `lib/data/fake_daemon_client.dart` | Client daemon simulé pour tests/dev sans daemon |
| `lib/data/dto/rpc_dto.dart` | Objets de transfert de données pour le protocole RPC |
| `lib/domain/services/mule_daemon_client.dart` | Interface abstraite du client daemon (contrat) |
| `lib/domain/models/category_item.dart` | Modèle : catégorie de téléchargement |
| `lib/domain/models/daemon_event.dart` | Modèle : événement émis par le daemon |
| `lib/domain/models/daemon_info.dart` | Modèle : informations d'état du daemon |
| `lib/domain/models/download_item.dart` | Modèle : élément de téléchargement |
| `lib/domain/models/filter_models.dart` | Modèles : filtres (IP filter, etc.) |
| `lib/domain/models/history_entry.dart` | Modèle : entrée d'historique |
| `lib/domain/models/history_stats.dart` | Modèle : statistiques d'historique |
| `lib/domain/models/lab_models.dart` | Modèles : fonctionnalités "lab" expérimentales |
| `lib/domain/models/network_policy_models.dart` | Modèles : politique réseau |
| `lib/domain/models/part_met_models.dart` | Modèles : métadonnées de fichiers .part.met |
| `lib/domain/models/server_models.dart` | Modèles : serveurs eD2k |
| `pubspec.yaml` | Manifeste du package |
| `test/fake_daemon_client_test.dart` | Tests du client simulé |
| `test/rpc_daemon_client_test.dart` | Tests du client RPC |
| `README.md` | Usage, protocole et comportements transport du client |

## packages/ui_kit/ — Design system partagé

| Fichier | Fonction |
|---|---|
| `lib/ui_kit.dart` | Barrel : export unique du package |
| `lib/src/theme.dart` | `AppTheme` : couleurs brand/status, `dark()` ThemeData |
| `lib/src/tokens.dart` | Design tokens (espacements, rayons, couleurs) |
| `lib/src/formatters.dart` | `Formatters` : bytes, vitesses, ETA, pourcentages |
| `lib/src/strings.dart` | `AppStrings` : chaînes UI centralisées (base i18n) |
| `lib/src/part_availability_bar.dart` | Bandeau eMule par part (rouge/bleu/teal) |
| `pubspec.yaml` | Manifeste du package |
| `README.md` | Contenu et règles du design system |

## crates/ — Crates Rust

### `mule-cli` — Interface en ligne de commande

| Fichier | Fonction |
|---|---|
| `src/main.rs` | Point d'entrée du binaire CLI |
| `src/lib.rs` | Bibliothèque du CLI |
| `src/commands/mod.rs` | Module des sous-commandes |
| `src/commands/hash.rs` | Commande de calcul de hash ed2k |
| `src/commands/link.rs` | Commande de génération de liens ed2k |
| `src/commands/part_met.rs` | Commande d'inspection de fichiers .part.met |
| `tests/make_link_tests.rs` | Tests de la commande de création de liens |

### `mule-core` — Modèle de domaine et persistance

| Fichier | Fonction |
|---|---|
| `src/lib.rs` | Racine de la bibliothèque |
| `src/error.rs` | Types d'erreurs du crate |
| `src/model.rs` | Modèles de données centraux |
| `src/kad/mod.rs` | Module Kademlia |
| `src/kad/contact.rs` | Représentation d'un contact Kad |
| `src/kad/kad_id.rs` | Identifiant Kad (128 bits) |
| `src/kad/keyword_index.rs` | Index des mots-clés Kad |
| `src/kad/routing_table.rs` | Table de routage Kad (k-buckets) |
| `src/kad/source_index.rs` | Index des sources Kad |
| `src/kad/note_index.rs` | Index des notes Kad |
| `src/persistence/mod.rs` | Module de persistance |
| `src/persistence/traits.rs` | Traits des magasins de données |
| `src/persistence/sqlite/mod.rs` | Implémentation SQLite |
| `src/persistence/sqlite/schema.rs` | Schéma de la base SQLite |
| `src/persistence/sqlite/categories.rs` | Persistance des catégories |
| `src/persistence/sqlite/credits.rs` | Persistance des crédits (upload/download) |
| `src/persistence/sqlite/downloads.rs` | Persistance des téléchargements |
| `src/persistence/sqlite/friends.rs` | Persistance des amis (friends.met) |
| `src/persistence/sqlite/history.rs` | Persistance de l'historique |
| `src/persistence/sqlite/kad.rs` | Persistance des données Kad |
| `src/persistence/sqlite/servers.rs` | Persistance des serveurs |
| `src/persistence/sqlite/shares.rs` | Persistance des fichiers partagés |
| `src/persistence/sqlite/sources.rs` | Persistance des sources/pairs |
| `src/ranges/mod.rs` | Module de gestion d'intervalles |
| `src/ranges/range_set.rs` | Ensemble d'intervalles (parties téléchargées) |
| `src/storage/mod.rs` | Module de stockage des fichiers |
| `src/storage/finalizer.rs` | Finalisation des fichiers complets |
| `src/storage/part_file.rs` | Gestion des fichiers .part |
| `src/verification/mod.rs` | Module de vérification d'intégrité |
| `src/verification/aich.rs` | Vérification AICH (hash tree) |
| `src/verification/ed2k.rs` | Vérification des hash ed2k |

### `mule-core` — Tests

| Fichier | Fonction |
|---|---|
| `tests/k_buckets_tests.rs` | Tests des k-buckets Kad |
| `tests/part_file_and_persistence.rs` | Tests fichiers .part + persistance |
| `tests/range_set.rs` | Tests du RangeSet |
| `tests/range_set_properties.rs` | Tests de propriétés du RangeSet |
| `tests/sqlite_categories_tests.rs` | Tests SQLite catégories |
| `tests/sqlite_corruption.rs` | Tests de récupération de corruption SQLite |
| `tests/sqlite_history_tests.rs` | Tests SQLite historique |
| `tests/sqlite_kad_keywords_tests.rs` | Tests SQLite mots-clés Kad |
| `tests/sqlite_kad_sources_tests.rs` | Tests SQLite sources Kad |
| `tests/sqlite_peer_sources_tests.rs` | Tests SQLite sources de pairs |
| `tests/sqlite_servers_tests.rs` | Tests SQLite serveurs |
| `tests/sqlite_settings_tests.rs` | Tests SQLite paramètres |

### `mule-crypto` — Cryptographie legacy eMule

| Fichier | Fonction |
|---|---|
| `Cargo.toml` | Manifeste du crate |
| `src/lib.rs` | Racine du crate (MD4, identifiants) |
| `src/cryptkey.rs` | Format `cryptkey.dat` (DER + Base64 Crypto++) |
| `src/legacy_sui.rs` | Secure User Identification (RSA-384 PKCS#1 v1.5 + SHA-1) |

### `mule-daemon` — Daemon principal

| Fichier | Fonction |
|---|---|
| `src/main.rs` | Point d'entrée du daemon |
| `src/lib.rs` | Racine de la bibliothèque daemon |
| `src/config.rs` | Chargement de la configuration |
| `src/error.rs` | Erreurs du daemon |
| `src/lock.rs` | Verrou d'instance unique |
| `src/session.rs` | Gestion de la session du daemon |
| `src/state.rs` | État partagé du daemon — ports effectifs (`ed2k_port`/`kad_port`/`udp_enabled` en atomics), `effective_obfuscation()`, `current_nickname()`, restauration des réglages persistés au boot |
| `src/workdir.rs` | Gestion du répertoire de travail |
| `src/ed2k/mod.rs` | Module eD2k du daemon |
| `src/ed2k/downloader.rs` | Téléchargeur eD2k |
| `src/lab/mod.rs` | Module lab (expérimental) |
| `src/lab/downloader.rs` | Téléchargeur lab |
| `src/lab/source_service.rs` | Service de sources lab |

### `mule-daemon` — RPC

| Fichier | Fonction |
|---|---|
| `src/rpc/mod.rs` | Module RPC |
| `src/rpc/server.rs` | Serveur RPC (transport) |
| `src/rpc/methods.rs` | Définition des méthodes RPC |
| `src/rpc/types.rs` | Types du protocole RPC |
| `src/rpc/events.rs` | Événements RPC (notifications) |
| `src/rpc/handlers/mod.rs` | Module des handlers RPC |
| `src/rpc/handlers/categories.rs` | Handler : catégories |
| `src/rpc/handlers/downloads.rs` | Handler : téléchargements |
| `src/rpc/handlers/friends.rs` | Handler : amis (friend slots) |
| `src/rpc/handlers/history.rs` | Handler : historique |
| `src/rpc/handlers/kad.rs` | Handler : Kad |
| `src/rpc/handlers/lab.rs` | Handler : lab |
| `src/rpc/handlers/network.rs` | Handler : politique réseau |
| `src/rpc/handlers/servers.rs` | Handler : serveurs |
| `src/rpc/handlers/settings.rs` | Handler : paramètres — `settings.get`/`set` couvrant tous les réglages runtime (ports, UDP, UPnP, obfuscation, capacités — clamp eMule 90 %/80 % sur les limites, USS, quotas, file upload) avec application à chaud (`apply_port_change`, `apply_udp_state`, `apply_capacity_clamp`) ; write-probe sur les dossiers, warning sur clés inconnues |
| `src/rpc/handlers/shares.rs` | Handler : fichiers partagés — `shares.list` (incl. partiels), `add`, `remove`, `setEnabled`, `setUploadPriority` (mutations refusées sur `is_partial`), `rescan` |
| `src/rpc/handlers/sources.rs` | Handler : sources |
| `src/rpc/handlers/upnp.rs` | Handler : UPnP |

### `mule-daemon` — Services

| Fichier | Fonction |
|---|---|
| `src/services/mod.rs` | Module des services |
| `src/services/incoming_share.rs` | Scan du dossier Incoming (démarrage + `shares.rescan` manuel) : hash eD2K + partage automatique (opt-out `share_download_dir`) |
| `src/services/downloads.rs` | Service de gestion des téléchargements |
| `src/services/downloads/history_categories.rs` | Historique et catégories des downloads |
| `src/services/downloads/operations.rs` | Opérations sur les téléchargements (pause, reprise…) |
| `src/services/downloads/part_met.rs` | Import/gestion des .part.met |
| `src/services/downloads/peers_servers.rs` | Sources pairs et serveurs des downloads |
| `src/services/downloads/types.rs` | Types du service downloads |
| `src/services/events.rs` | Bus d'événements internes |
| `src/services/identity.rs` | Identité du client (user hash, ident) |
| `src/services/inbound_listener.rs` | Écoute des connexions entrantes (obfuscation live via `set_obfuscation`) |
| `src/services/inbound_listener/connection.rs` | Acceptation de connexions entrantes |
| `src/services/inbound_listener/dispatcher.rs` | Dispatch des connexions entrantes |
| `src/services/inbound_listener/types.rs` | Types du listener |
| `src/services/kad.rs` | Service Kad |
| `src/services/kad/nodes_dat.rs` | Bootstrap via nodes.dat |
| `src/services/kad/search_keywords.rs` | Recherche par mots-clés Kad |
| `src/services/kad/search_sources.rs` | Recherche de sources Kad |
| `src/services/kad/types.rs` | Types du service Kad |
| `src/services/kad/firewall.rs` | Gestion du pare-feu Kad (FIREWALLED_UDP, tests de connectivité) |
| `src/services/kad_udp.rs` | Service Kad UDP |
| `src/services/kad_udp/handlers.rs` | Handlers des paquets Kad UDP |
| `src/services/kad_udp/routing_client.rs` | Client de routage Kad |
| `src/services/kad_udp/search_client.rs` | Client de recherche Kad |
| `src/services/client_udp.rs` | Socket client-UDP eD2K (reasks/keep-alive 0x90-0x93) |
| `src/services/server_udp.rs` | Socket UDP serveur (GLOBGETSOURCES2/GLOBFOUNDSOURCES) + failover dans `swarm_coordinator/strategy.rs` |
| `src/services/uss.rs` | USS — limite upload dynamique par latence (loi `LastCommonRouteFinder`, gate `accept_new_client` sur la file) |
| `src/services/kad_udp/storage.rs` | Nœud de stockage Kad (TTL, bornes anti-flood) |
| `src/services/kad_udp/types.rs` | Types Kad UDP |
| `src/services/obfuscation_stream.rs` | Flux réseau obfusqué (protocol obfuscation) |
| `src/services/peer_session.rs` | Session avec un pair eD2k |
| `src/services/peer_session/connection.rs` | Connexion au pair |
| `src/services/peer_session/events.rs` | Événements de session pair |
| `src/services/peer_session/handshake.rs` | Handshake eD2k avec le pair |
| `src/services/peer_session/source_exchange.rs` | Échange de sources entre pairs |
| `src/services/peer_session/status_query.rs` | Requêtes de statut de fichier |
| `src/services/peer_session/sui.rs` | Secure User Identification dans la session pair |
| `src/services/peer_session/transfer.rs` | Transfert de blocs avec le pair |
| `src/services/peer_session/types.rs` | Types de session pair |
| `src/services/server_session.rs` | Session avec un serveur eD2k |
| `src/services/server_session/connection.rs` | Connexion au serveur |
| `src/services/server_session/events.rs` | Événements de session serveur |
| `src/services/server_session/types.rs` | Types de session serveur |
| `src/services/swarm_allocator.rs` | Allocation des ressources dans l'essaim |
| `src/services/swarm_coordinator.rs` | Coordination de l'essaim de pairs |
| `src/services/swarm_coordinator/strategy.rs` | Stratégie de coordination (A4AF rebalance inclus) |
| `src/services/swarm_coordinator/types.rs` | Types du coordinateur |
| `src/services/upload_queue.rs` | File d'attente d'upload (trickle slots, friend slots, anti-leecher, taille configurable `set_max_queue`) |
| `src/services/upnp.rs` | Service UPnP (`set_enabled`, `set_ports`, map/unmap, refresh périodique) |
| `src/services/upnp/discovery.rs` | Découverte de la passerelle UPnP |
| `src/services/upnp/mapping.rs` | Mappage de ports UPnP |
| `src/services/upnp/types.rs` | Types UPnP |

### `mule-daemon` — Tests

| Fichier | Fonction |
|---|---|
| `tests/category_rpc_tests.rs` | Tests RPC catégories |
| `tests/daemon_adversarial.rs` | Tests adversariaux du daemon |
| `tests/download_details_and_destination_tests.rs` | Tests détails/destination des downloads |
| `tests/dual_portable_bundles.rs` | Tests de bundles portables doubles |
| `tests/ed2k_obfuscation_tests.rs` | Tests d'obfuscation eD2k |
| `tests/ed2k_source_discovery.rs` | Tests de découverte de sources eD2k |
| `tests/ed2k_transfer_tests.rs` | Tests de transfert eD2k |
| `tests/filter_rpc_tests.rs` | Tests RPC de filtrage |
| `tests/history_rpc_tests.rs` | Tests RPC historique |
| `tests/inter_daemon_transfer.rs` | Tests de transfert entre daemons |
| `tests/aich_tests.rs` | Tests AICH daemon (consensus, recovery) |
| `tests/hashset_tests.rs` | Tests vérification hashset par part |
| `tests/kad_bootstrap_tests.rs` | Tests de bootstrap Kad |
| `tests/kad_keywords_tests.rs` | Tests mots-clés Kad |
| `tests/kad_rpc_tests.rs` | Tests RPC Kad |
| `tests/kad_sources_tests.rs` | Tests sources Kad |
| `tests/kad_udp_tests.rs` | Tests Kad UDP |
| `tests/lab_rpc_tests.rs` | Tests RPC lab |
| `tests/lifecycle_and_locking.rs` | Tests cycle de vie et verrouillage |
| `tests/network_policy_rpc_tests.rs` | Tests RPC politique réseau |
| `tests/part_met_rpc_tests.rs` | Tests RPC part.met |
| `tests/part_met_write_tests.rs` | Tests écriture .part.met |
| `tests/peer_adversarial_tests.rs` | Tests adversariaux de pairs |
| `tests/peer_connection_tests.rs` | Tests de connexion aux pairs |
| `tests/rpc_contract.rs` | Tests de contrat RPC |
| `tests/secure_ident_tests.rs` | Tests d'identification sécurisée |
| `tests/server_connection_tests.rs` | Tests de connexion aux serveurs |
| `tests/servers_rpc_tests.rs` | Tests RPC serveurs |
| `tests/swarm_dual_peer_tests.rs` | Tests essaim à deux pairs |
| `tests/upload_inbound_tests.rs` | Tests upload/connexions entrantes |
| `tests/upnp_and_wan_tests.rs` | Tests UPnP et WAN |
| `tests/workdir_tests.rs` | Tests du répertoire de travail |

### `mule-filter` — Filtrage IP et limitation de débit

| Fichier | Fonction |
|---|---|
| `src/lib.rs` | Racine du crate |
| `src/filter.rs` | Moteur de filtrage IP |
| `src/interval.rs` | Intervalles d'adresses IP |
| `src/parser.rs` | Parseur de fichiers ipfilter.dat |
| `src/pseudonym.rs` | Support des pseudonymes |
| `src/token_bucket.rs` | Token bucket (limitation de débit) |
| `tests/filter_tests.rs` | Tests du filtrage |
| `tests/token_bucket_tests.rs` | Tests du token bucket |

### `mule-format` — Formats de fichiers eMule

| Fichier | Fonction |
|---|---|
| `src/lib.rs` | Racine du crate |
| `src/error.rs` | Erreurs de format |
| `src/fixtures.rs` | Données de test réutilisables |
| `src/aich/mod.rs` | Module AICH |
| `src/aich/hash_tree.rs` | Arbre de hachage AICH |
| `src/aich/proof.rs` | Preuves AICH |
| `src/aich/recovery.rs` | Données de récupération AICH (ident+hash) |
| `src/ed2k/mod.rs` | Module formats eD2k |
| `src/ed2k/hash.rs` | Calcul du hash eD2k (MD4 par chunks) |
| `src/ed2k/link.rs` | Parsing/génération de liens ed2k:// |
| `src/ed2k/source.rs` | Représentation d'une source ed2k |
| `src/kad/mod.rs` | Module formats Kad |
| `src/kad/nodes_dat.rs` | Lecture/écriture de nodes.dat |
| `src/met/mod.rs` | Module fichiers .met |
| `src/met/error.rs` | Erreurs de format .met |
| `src/met/part_met.rs` | Structure des fichiers .part.met |
| `src/met/reader.rs` | Lecteur de fichiers .met |
| `src/met/server_met.rs` | Parsing de server.met |
| `src/met/tags.rs` | Tags des fichiers .met |
| `tests/aich_vectors.rs` | Vecteurs de test AICH |
| `tests/ed2k_hash_vectors.rs` | Vecteurs de test hash eD2k |
| `tests/link_vectors.rs` | Vecteurs de test liens ed2k |
| `tests/nodes_dat_tests.rs` | Tests nodes.dat |
| `tests/official_file_hash.rs` | Test de hash sur fichier officiel |
| `tests/part_met_vectors.rs` | Vecteurs de test .part.met (lecture/écriture) |
| `tests/part_met_vectors.rs` | Vecteurs de test .part.met |
| `tests/server_met_vectors.rs` | Vecteurs de test server.met |

### `mule-mcp` — Serveur MCP (intégration agents IA)

| Fichier | Fonction |
|---|---|
| `src/main.rs` | Point d'entrée du serveur MCP |
| `src/lib.rs` | Racine du crate |
| `src/client.rs` | Client vers le daemon |
| `src/protocol.rs` | Protocole MCP (JSON-RPC) |
| `src/tools/mod.rs` | Module des outils MCP |
| `src/tools/downloads.rs` | Outils MCP : téléchargements |
| `src/tools/downloads/handlers.rs` | Handlers des outils downloads |
| `src/tools/downloads/schemas.rs` | Schémas JSON des outils downloads |
| `src/tools/kad.rs` | Outils MCP : Kad |
| `src/tools/servers.rs` | Outils MCP : serveurs |
| `src/tools/shares.rs` | Outils MCP : partages |
| `src/tools/shares/handlers.rs` | Handlers des outils shares |
| `src/tools/shares/schemas.rs` | Schémas JSON des outils shares |
| `src/tools/system.rs` | Outils MCP : système/statut |
| `tests/mcp_protocol_tests.rs` | Tests du protocole MCP |

### `mule-network-policy` — Politique réseau

| Fichier | Fonction |
|---|---|
| `src/lib.rs` | Racine du crate |
| `src/audit.rs` | Journal d'audit des décisions réseau |
| `src/budgets.rs` | Budgets réseau (bande passante) |
| `src/cancellation.rs` | Annulation d'opérations réseau |
| `src/manager.rs` | Gestionnaire de politique réseau |
| `src/permit.rs` | Permis d'accès réseau |
| `src/profile.rs` | Profils de politique (lab, WAN…) |
| `src/state.rs` | État de la politique |
| `src/validator.rs` | Validateur de politique |
| `tests/policy_validation_tests.rs` | Tests de validation de politique |

### `mule-protocol` — Protocoles eD2k / Kad

#### Framing & base

| Fichier | Fonction |
|---|---|
| `AGENTS.md` | Règles agents spécifiques à la crate (référence eMule 0.72a `srchybrid`, bornes `limits.rs`, déterminisme wire) |
| `src/lib.rs` | Racine du crate |
| `src/error.rs` | Erreurs de protocole |
| `src/limits.rs` | Limites globales du protocole |
| `src/traits.rs` | Traits communs (encode/decode) |
| `src/types.rs` | Types communs |
| `src/framing/mod.rs` | Module de cadrage des paquets |
| `src/framing/codec.rs` | Codec de cadrage (en-têtes, tailles) |
| `src/framing/packet.rs` | Structure de paquet brut |

#### eD2k — encodage/décodage

| Fichier | Fonction |
|---|---|
| `src/ed2k/mod.rs` | Module protocole eD2k |
| `src/ed2k/client_udp.rs` | Codec client-UDP (reasks 0x90-0x93) |
| `src/ed2k/server_udp.rs` | Codec UDP serveur (GLOBGETSOURCES2, GLOBFOUNDSOURCES, GLOBSERVSTATRES) |
| `src/ed2k/opcodes.rs` | Opcodes des messages eD2k |
| `src/ed2k/packet.rs` | Paquet eD2k |
| `src/ed2k/tags.rs` | Tags eD2k (métadonnées typées) |
| `src/ed2k/peer_info.rs` | Informations sur un pair |
| `src/ed2k/profile.rs` | Profil client eD2k |
| `src/ed2k/protocol.rs` | Définition du protocole eD2k |
| `src/ed2k/reference.rs` | Interop avec l'implémentation de référence |
| `src/ed2k/limits.rs` | Limites spécifiques eD2k |
| `src/ed2k/loopback.rs` | Transport loopback pour tests locaux |
| `src/ed2k/error.rs` | Erreurs eD2k |
| `src/ed2k/decoder.rs` | Décodeur eD2k (profil `Emule072a` par défaut, `extended_requests_version` négocié) |
| `src/ed2k/decoder/handshake.rs` | Décodage des messages de handshake |
| `src/ed2k/decoder/security.rs` | Décodage des messages de sécurité |
| `src/ed2k/decoder/status.rs` | Décodage des messages de statut |
| `src/ed2k/decoder/transfers.rs` | Décodage des messages de transfert |
| `src/ed2k/decoder/aich.rs` | Décodage AICH (`0x9B`–`0x9E`) |
| `src/ed2k/decoder/extended.rs` | Décodage eMule 0.72a étendu (HashSet2, MultiPacket*, queue, chat, CAPTCHA) |
| `src/ed2k/encoder.rs` | Encodeur eD2k |
| `src/ed2k/encoder/handshake.rs` | Encodage du handshake |
| `src/ed2k/encoder/security.rs` | Encodage sécurité |
| `src/ed2k/encoder/tags.rs` | Encodage des tags |
| `src/ed2k/encoder/transfers.rs` | Encodage des transferts |
| `src/ed2k/encoder/aich.rs` | Encodage AICH |
| `src/ed2k/encoder/extended.rs` | Encodage eMule 0.72a étendu |
| `src/ed2k/file_identifier.rs` | `Ed2kFileIdentifier` (référence fichier eMule 0.72a : hash, taille, AICH) |

#### eD2k — obfuscation

| Fichier | Fonction |
|---|---|
| `src/ed2k/obfuscation/mod.rs` | Module obfuscation de protocole |
| `src/ed2k/obfuscation/dh.rs` | Échange de clés Diffie-Hellman |
| `src/ed2k/obfuscation/framing.rs` | Cadrage des trames obfusquées |
| `src/ed2k/obfuscation/keys.rs` | Dérivation de clés |
| `src/ed2k/obfuscation/rc4.rs` | Chiffrement RC4 |

#### eD2k — sessions, serveur, échanges

| Fichier | Fonction |
|---|---|
| `src/ed2k/session.rs` | Session eD2k (machine à états) |
| `src/ed2k/session/tracker.rs` | Suivi d'état de session |
| `src/ed2k/session/types.rs` | Types de session |
| `src/ed2k/sequencer.rs` | Séquenceur de blocs (least-received, preview first/last, end-game redundancy) |
| `src/ed2k/sequencer/sink.rs` | Récepteur de blocs séquencés |
| `src/ed2k/sequencer/types.rs` | Types du séquenceur |
| `src/ed2k/server.rs` | Protocole client↔serveur |
| `src/ed2k/server/search.rs` | Recherche sur serveur |
| `src/ed2k/server/sources.rs` | Requête de sources au serveur |
| `src/ed2k/server/types.rs` | Types serveur — `OfferedFile::for_server` : marqueurs `0xFBFB` (complet) / `0xFCFC` (partiel `is_partial`) quand `SRV_TCPFLG_COMPRESSION` |
| `src/ed2k/source_exchange.rs` | Échange de sources pair-à-pair |
| `src/ed2k/source_exchange/codec.rs` | Codec de l'échange de sources |
| `src/ed2k/source_exchange/types.rs` | Types de l'échange de sources |
| `src/ed2k/storage.rs` | Interface de stockage des blocs reçus |
| `src/ed2k/storage/sink.rs` | Écriture des blocs |
| `src/ed2k/storage/types.rs` | Types de stockage |

#### Kad

| Fichier | Fonction |
|---|---|
| `src/kad/mod.rs` | Module protocole Kad |
| `src/kad/codec.rs` | Codec Kad |
| `src/kad/opcodes.rs` | Opcodes Kad |
| `src/kad/types.rs` | Types Kad |
| `src/kad/decoder.rs` | Décodeur Kad |
| `src/kad/decoder/routing.rs` | Décodage des messages de routage |
| `src/kad/decoder/search.rs` | Décodage des messages de recherche |
| `src/kad/encoder.rs` | Encodeur Kad |
| `src/kad/udp_crypt.rs` | Chiffrement des paquets Kad UDP |

#### Lab (protocole expérimental)

| Fichier | Fonction |
|---|---|
| `src/lab/mod.rs` | Module lab |
| `src/lab/in_flight.rs` | Suivi des blocs en vol |
| `src/lab/messages.rs` | Messages du protocole lab |
| `src/lab/source_engine.rs` | Moteur de sources lab |
| `src/lab/state_machine.rs` | Machine à états lab |

#### `mule-protocol` — Tests et fixtures

| Fichier | Fonction |
|---|---|
| `tests/common/mod.rs` | Utilitaires de test partagés |
| `tests/common/sha256.rs` | SHA-256 pour les tests |
| `tests/ed2k_aich_tests.rs` | Tests codec AICH (opcodes 0x9B-0x9E) |
| `tests/ed2k_block_transfer_tests.rs` | Tests de transfert de blocs eD2k |
| `tests/ed2k_corpus_tests.rs` | Tests sur corpus de paquets |
| `tests/ed2k_emule_compat_tests.rs` | Tests de compatibilité filaire eMule |
| `tests/ed2k_extended_tests.rs` | Tests codecs eMule 0.72a étendus (HashSet2, MultiPacket*, ext-info par version négociée, CAPTCHA…) |
| `tests/ed2k_file_status_tests.rs` | Tests des statuts de fichier |
| `tests/ed2k_hashset_tests.rs` | Tests codec hashset (0x51/0x52) |
| `tests/ed2k_isolation_guard.rs` | Garde d'isolation des tests eD2k |
| `tests/ed2k_part_resume_tests.rs` | Tests de reprise de parties |
| `tests/ed2k_reference_interop_tests.rs` | Tests d'interop avec la référence |
| `tests/ed2k_roundtrip_tests.rs` | Tests roundtrip encode/decode |
| `tests/ed2k_sequencer_tests.rs` | Tests du séquenceur |
| `tests/ed2k_session_tests.rs` | Tests de session eD2k |
| `tests/in_flight_properties.rs` | Propriétés du suivi in-flight |
| `tests/kad_codec_tests.rs` | Tests du codec Kad |
| `tests/lab_adversarial.rs` | Tests adversariaux lab |
| `tests/lab_in_flight_and_blocks.rs` | Tests in-flight et blocs lab |
| `tests/lab_state_machine.rs` | Tests machine à états lab |
| `tests/loopback_transfer.rs` | Tests de transfert en loopback |
| `tests/malformed_packets.rs` | Tests de paquets malformés |
| `tests/packet_codec.rs` | Tests du codec de paquets |
| `tests/protocol_limits.rs` | Tests des limites du protocole |
| `tests/server_codec_tests.rs` | Tests du codec serveur |
| `tests/tier3_loopback.rs` | Tests loopback tier 3 |
| `tests/tier3_external_reference/mod.rs` | Module de tests de référence externe |
| `tests/tier3_external_reference/environment.rs` | Environnement de test externe |
| `tests/tier3_external_reference/harness.rs` | Harnais de test externe |
| `tests/tier3_external_reference/report.rs` | Rapport des tests externes |
| `tests/tier3_external_reference/README.md` | Doc des tests de référence externe |
| `tests/fixtures/ed2k/README.md` | Doc des fixtures eD2k |
| `tests/fixtures/ed2k/*.bin` | Paquets binaires capturés (hello, file_status, request_parts, sending_part…) |
| `tests/fixtures/ed2k/malformed/*.bin` | Paquets malformés pour tests négatifs |
| `tests/fixtures/ed2k/manifest.json` | Manifeste décrivant les fixtures |
| `tests/fixtures/ed2k/reference_manifest.json` | Manifeste de référence |

## docs/ — Documentation

### Racine de la documentation
| Fichier | Fonction |
|---|---|
| `docs/README.md` | Portail d'orientation de la documentation technique |
| `docs/INDEX.md` | Index exhaustif des fichiers du dépôt |
| `docs/CHANGELOG.md` | Journal des modifications (Keep a Changelog) |
| `docs/SETTINGS.md` | Référence complète des paramètres (`settings.get`/`set`) : clés RPC/persistées, défauts, application à chaud, équivalents eMule |

### docs/architecture/ — Architecture & Rapports
| Fichier | Fonction |
|---|---|
| `docs/architecture/rapport_architecture_et_avancement.md` | Rapport détaillé d'architecture logicielle, Clean Architecture et état d'avancement |
| `docs/architecture/rpc_protocol.md` | Contrat JSON-RPC 2.0/WebSocket daemon↔clients : transport, auth, méthodes, events, codes d'erreur, versioning |
| `docs/architecture/plan_ui_multiplateforme.md` | Plan UI/UX multiplateforme (desktop, mobile, web) : phases, packages, architecture |
| `docs/architecture/plan_daemon_events.md` | Plan événements daemon : uploads temps réel, statsTick, cycle de vie downloads, fin du polling (phases E1–E5) |
| `docs/architecture/conventions_ui.md` | Conventions clean architecture UI : couches, 1 widget par fichier, design tokens, navigation, état |

### docs/walkthroughs/ — Guides d'Exécution & Interopérabilité par Phase
| Fichier | Fonction |
|---|---|
| `docs/walkthroughs/README.md` (ou `INDEX.md`) | Répertoire maître et table d'orientation de tous les walkthroughs |
| `docs/walkthroughs/walkthrough_Phase_01_a_03c.md` | Guide d'exécution et durcissement produit (Phases 1 à 3C) |
| `docs/walkthroughs/walkthrough_Tier3_eMule050a.md` | Interopérabilité loopback avec eMule 0.50a officiel (Tier 3.1 à 3.6) |
| `docs/walkthroughs/walkthrough_Phase_07.md` | Mode portable, isolation `data/` vs `downloads/` et persistance SQLite (Phase 7) |
| `docs/walkthroughs/walkthrough_Phase_08.md` | Part Map interactive, historique local, catégories et import .part.met (Phase 8) |
| `docs/walkthroughs/walkthrough_Phase_09.md` | Politique réseau défensive 7 couches, permis RAII, Kill Switch et IP Filter (Phase 9) |
| `docs/walkthroughs/walkthrough_Phase_10.md` | Infrastructure serveurs eD2K, TCP Lugdunum, HighID/LowID, recherche et sources (Phase 10) |
| `docs/walkthroughs/walkthrough_Phase_11.md` | Swarm multi-sources, allocateur Water-Filling et démarrage basse latence (Phase 11) |
| `docs/walkthroughs/walkthrough_Phase_12.md` | Fondations Kad DHT, KadId 128-bit, K-Buckets et transport UDP (Phase 12) |
| `docs/walkthroughs/walkthrough_Phase_13.md` | Source Exchange v4 (SX1/SX2), compression zlib et serveur MCP 83 outils (Phase 13) |
| `docs/walkthroughs/walkthrough_Phase_14.md` | Convergence recherche Kad DHT (CUInt128) et Clean Architecture complète (Phase 14) |
| `docs/walkthroughs/walkthrough_Phase_15.md` | Swarm multi-pairs réel, conformité files d'attente eMule et logs stratégiques (Phase 15) |

### docs/releases/ — Notes de Version Détaillées
| Fichier | Fonction |
|---|---|
| `docs/releases/RELEASE_NOTES_0.1.0-lab.md` | Notes de version 0.1.0-lab |
| `docs/releases/RELEASE_NOTES_0.1.1-lab.md` | Notes de version 0.1.1-lab |
| `docs/releases/RELEASE_NOTES_0.1.2-lab.md` | Notes de version 0.1.2-lab |
| `docs/releases/RELEASE_NOTES_0.1.3-lab.md` | Notes de version 0.1.3-lab |
| `docs/releases/RELEASE_NOTES_0.1.8.md` | Notes de version 0.1.8 |

### docs/qa/ — Assurance Qualité & Conformité
| Fichier | Fonction |
|---|---|
| `docs/qa/QA_RECEPTION_PROTOCOL.md` | Protocole de réception QA |
| `docs/qa/compatibility-matrix.md` | Matrice de compatibilité |
| `docs/qa/test-vectors.md` | Vecteurs de test documentés |

### docs/ed2k/ — Spécifications Filaire eD2K & ADRs
| `ed2k/compatibility-notes.md` | Notes de compatibilité eD2k |
| `ed2k/framing.md` | Documentation du cadrage eD2k |
| `ed2k/limits.md` | Limites du protocole eD2k |
| `ed2k/message-catalog.md` | Catalogue des messages eD2k |
| `ed2k/opcodes.md` | Liste des opcodes eD2k |
| `ed2k/opcodes_matrix.md` | Matrice des opcodes |
| `ed2k/references.md` | Références externes |
| `ed2k/scope.md` | Périmètre de l'implémentation eD2k |
| `ed2k/SUI_WIRE_FORMAT.md` | Format filaire SUI (Secure User Identification), citations eMule 0.72a vérifiées |
| `ed2k/SUI_SESSION_RECAP.md` | Récapitulatif des sessions de travail SUI |
| `ed2k/messages/op_filestatus.md` | Spec du message OP_FILESTATUS |
| `ed2k/messages/op_filestatusreq.md` | Spec du message OP_FILESTATUSREQ |
| `ed2k/messages/op_requestparts.md` | Spec du message OP_REQUESTPARTS |
| `ed2k/messages/op_sendingpart.md` | Spec du message OP_SENDINGPART |
| `ed2k/decisions/0001-ed2k-read-only.md` | ADR : eD2k en lecture seule |
| `ed2k/decisions/0002-no-public-network.md` | ADR : pas de réseau public |
| `ed2k/decisions/0003-wire-lab-separation.md` | ADR : séparation wire/lab |

## scripts/ — Scripts PowerShell

| Fichier | Fonction |
|---|---|
| `build_windows_release.ps1` | Build de release Windows |
| `generate_sbom.ps1` | Génération du SBOM (Bill of Materials) |
| `test_packaged_bundle.ps1` | Test du bundle packagé |
| `test_qa_loopback_e2e.ps1` | Test E2E loopback QA |
| `test_rc_matrix.ps1` | Matrice de tests release candidate |
| `verify_all.ps1` | Script de vérification globale |
