# Guide d'utilisation — Assistant IA local (mule-assistant)

`mule-assistant` permet à un **modèle de langage local** (Ollama, ou tout
backend compatible `/api/chat`) de piloter eMule-Rust à travers les outils
MCP existants. Le daemon reste l'autorité unique : aucune logique métier
n'est dupliquée, et le modèle ne voit qu'un sous-ensemble curé et validé
des 87 outils.

```
Ollama (127.0.0.1:11434)          Panneau Flutter
        │ chat + tool_calls               │ WS JSON-RPC
        ▼                                 ▼
            ┌─────── mule-assistant ───────┐
            │  policy : allowlist          │
            │         ∩ profil MCP         │
            │         ∩ capacités daemon   │
            └───────────┬──────────────────┘
                        │ DaemonClient (même token que le RPC)
                        ▼
                   mule-daemon  ← autorité de sécurité
```

---

## 1. Prérequis

| Composant | Requis |
| :--- | :--- |
| **Ollama** | Installé et lancé sur `http://127.0.0.1:11434` (défaut) |
| **Modèle** | Compatible *tool calling* Ollama — ex. `qwen3:8b` (défaut), `llama3.1`, `mistral-nemo`… |
| **Daemon** | `mule-daemon` en cours d'exécution, session active (fichier `auth.token` présent dans le workdir) |

```powershell
# Vérifier qu'Ollama répond et que le modèle est présent
ollama list
ollama pull qwen3:8b        # si absent
```

> **Note** : un modèle sans support du tool calling fonctionne en mode
> « chat simple » mais ne pourra pas exécuter d'actions — l'assistant
> répondra seulement en texte.

---

## 2. Démarrage rapide

### 2.1 Mode interactif (REPL) — le plus simple

```powershell
cargo run -p mule-assistant -- chat
```

```
session prête — 15 outils visibles (profil full)
assistant> quel est l'état du client ?
```

L'assistant appelle les outils nécessaires (affichés `[outil →]`),
affiche les résultats (`[résultat ←]`), puis la réponse finale en
streaming. `Ctrl+C` ou ligne vide pour quitter.

### 2.2 Mode one-shot

```powershell
cargo run -p mule-assistant -- prompt "liste mes téléchargements actifs"
```

Sans terminal interactif, les outils à confirmation sont **refusés par
défaut** (comportement sûr) — le modèle reçoit « annulé par
l'utilisateur » et adapte sa réponse.

### 2.3 Mode serveur — pour le panneau Flutter

```powershell
cargo run -p mule-assistant -- serve
# ws://127.0.0.1:4680/assistant
```

Laissez ce processus tourner, puis dans l'application Flutter ouvrez la
section **« Assistant IA »** (sidebar). Le panneau se connecte
automatiquement : le jeton de session est lu depuis
`<workdir>\auth.token` — le même secret que le RPC daemon, aucun mot de
passe supplémentaire.

---

## 3. Référence des options

| Option | Défaut | Rôle |
| :--- | :--- | :--- |
| `--url <ws://…>` | détection auto | URL RPC du daemon (`ws://127.0.0.1:4660/rpc`) |
| `--token <hex64>` | `auth.token` du workdir | Jeton de session daemon |
| `--workdir <chemin>` | résolution standard | Workdir contenant `session.json` / `auth.token` |
| `--app <chemin>` | — | Racine du bundle applicatif |
| `--endpoint <http://…>` | `http://127.0.0.1:11434` | URL du backend LLM — **IP littérale loopback requise** |
| `--model <nom>` | `qwen3:8b` | Modèle côté backend |
| `--allow-remote-endpoint` | off | Autorise explicitement un endpoint LLM non loopback |
| `--tool-profile <p>` | `full` | Plafond de risque : `read` · `safe` · `full` |
| `serve --port <n>` | `4680` | Port loopback du serveur WebSocket |

### Profils d'outils (`--tool-profile`)

Le profil plafonne ce que le modèle peut voir **et** invoquer — le
filtrage est revérifié à l'exécution, pas seulement à l'affichage.

| Profil | Contenu | Usage |
| :--- | :--- | :--- |
| `read` | Consultation uniquement (`status`, `list_*`, `get_*`, `search`…) | Découverte, démo, usage prudent |
| `safe` | `read` + mutations réversibles (`pause`, `resume`, `connect`…) — **sans** les destructifs | Usage quotidien |
| `full` | Tous les outils de l'allowlist | Exploitation complète (confirmations toujours actives) |

Le même flag existe côté serveur MCP externe : `mule-mcp --tool-profile
safe` protège aussi les hôtes MCP (Claude Desktop, Devin…) — c'est le
même code, un seul propriétaire du risque.

### Outils visibles par le modèle

L'ensemble effectif est l'intersection de trois filtres, **revérifiée
avant chaque exécution** :

```
outils MCP du profil  ∩  allowlist curée (15 outils)  ∩  capacités daemon
```

L'allowlist couvre les opérations usuelles :

- **État** : `emule_status`, `emule_get_settings`
- **Recherche/réseau** : `emule_search`, `emule_list_servers`,
  `emule_connect_server`, `emule_disconnect_server`, `emule_kad_status`
- **Téléchargements** : `emule_list_downloads`,
  `emule_get_download_details`, `emule_add_link`, `emule_pause_download`,
  `emule_resume_download`
- **Partages/sources** : `emule_list_shares`, `emule_swarm_status`,
  `emule_list_sources`

Si une capacité est absente du daemon (ex. `server-search` désactivé),
l'outil correspondant est masqué au modèle.

### Confirmations utilisateur

Certains outils exigent une validation humaine **sur les arguments
exacts** proposés par le modèle :

- `emule_add_link` (ajoute un téléchargement — effet externe),
- `emule_connect_server` / `emule_disconnect_server`,
- **tout outil destructif** (dernier rempart, même en profil `full`).

En REPL : `o/n` interactif. Dans Flutter : bannière « L'assistant
demande confirmation » avec les arguments affichés, boutons
**Autoriser/Refuser**. Un refus devient un message d'erreur d'outil
pour le modèle — la conversation continue proprement.

---

## 4. Le panneau Flutter

Ouvrez **« Assistant IA »** dans la sidebar (icône robot). La vue affiche :

- **En-tête** : titre + pastille d'état (`Connecté · N outils`,
  `Connexion…`, `Erreur de connexion`, `Déconnecté`) ;
- **Conversation** : bulles utilisateur (droite) / assistant (gauche),
  tuiles d'outils (appel + résultat, contenu en monospace, tronqué à
  l'affichage si très long) ;
- **Streaming** : la réponse se compose token par token ; les appels
  d'outils apparaissent dès leur validation ;
- **Bannière de confirmation** quand un outil protégé est proposé ;
- **Bouton stop** pendant un tour pour annuler (`assistant.cancel`) ;
- **État hors-ligne** : explication + bouton « Réessayer » si le
  serveur assistant n'est pas lancé ou si le jeton est introuvable
  (daemon externe, mode web — la fonction est desktop).

> La conversation est partagée par serveur : une reconnexion retrouve le
> même historique (choix mono-utilisateur assumé).

---

## 5. Sécurité — ce qui est garanti

| Garantie | Mécanisme |
| :--- | :--- |
| Le modèle ne quitte pas la machine | Endpoint **IP littérale loopback** obligatoire (`127.0.0.1`/`::1`), noms d'hôte refusés, redirections HTTP désactivées, proxys d'environnement ignorés |
| Pas d'action hors scope | Allowlist fixe ∩ profil ∩ capacités, revérifiée à l'exécution — le modèle ne peut pas invoquer un outil non listé |
| Pas d'irréversible sans accord | Confirmations sur arguments exacts ; `Destructive` toujours confirmé |
| Même auth que le RPC | `assistant.hello` exige le jeton de session daemon ; le WS n'écoute que sur loopback |
| Boucle bornée | `MAX_TOOL_ROUNDS` = 4 enchaînements d'outils max par message |
| Contexte borné | Résultats d'outils tronqués à 4 000 caractères pour le modèle (l'UI reçoit l'intégral) |
| Sortie « thinking » masquée | Les fragments de réflexion du modèle ne sont jamais affichés |

**Limites connues** : les résultats P2P volumineux passent par les
événements daemon existants, pas par reformulation du modèle ; la
fiabilité du tool calling dépend du modèle — mesurez avec vos modèles
cibles (`RUST_LOG=mule_assistant=debug` expose `load_duration`,
`prompt_eval_duration`, `eval_duration` d'Ollama).

---

## 6. Protocole WebSocket (développeurs)

Endpoint : `ws://127.0.0.1:4680/assistant` — un objet JSON par trame.

```jsonc
// → authentification (premier appel obligatoire)
{"id":1,"method":"assistant.hello","params":{"token":"<hex64>"}}
// ← {"id":1,"result":{"server":"mule-assistant","version":"…","tools":15}}

// → message utilisateur
{"id":2,"method":"assistant.chat","params":{"text":"état du client ?"}}
// ← {"id":2,"result":{"accepted":true}}
// ← notifications ensuite :
//    assistant.event           {type:"token"|"toolStart"|"toolResult"|"final"|"notice"|"error", …}
//    assistant.confirmRequired {name, args}     → répondre par :
//    assistant.turnEnd         {error: null|string}

{"id":3,"method":"assistant.confirm","params":{"allow":true}}
{"id":4,"method":"assistant.cancel","params":{}}
```

Codes d'erreur : `-32001` jeton invalide · `-32002` auth requise ·
`-32003` tour déjà en cours · `-32004` aucune confirmation en attente ·
`-32005` aucun tour en cours.

Le client Dart `AssistantClient` (`packages/mule_client`) implémente ce
protocole — réutilisable pour d'autres frontaux.

---

## 7. Dépannage

| Symptôme | Cause probable | Résolution |
| :--- | :--- | :--- |
| « Assistant indisponible » dans Flutter | `mule-assistant serve` non lancé | Lancer le serveur, cliquer « Réessayer » |
| « Jeton de session introuvable » | Daemon externe ou web | L'assistant exige un daemon local supervisé (workdir lisible) |
| `endpoint … non loopback` | URL LLM distante ou nom d'hôte | Utiliser `http://127.0.0.1:11434` ; `--allow-remote-endpoint` si vraiment distant |
| Le modèle n'appelle jamais d'outil | Modèle sans tool calling | `ollama pull qwen3:8b` ou autre modèle compatible |
| `outil bloqué par le profil` | `--tool-profile read|safe` | Passer `--tool-profile full` ou choisir un autre outil |
| Réponses très lentes | Génération CPU | Modèle plus petit ; timeout LLM = 300 s (config) |
| `un tour est déjà en cours` | Double envoi | Attendre `turnEnd` ou `assistant.cancel` |

---

*Voir aussi : `docs/CHANGELOG.md` (entrée Unreleased),
`crates/mule-assistant/src/ws.rs` (protocole),
`crates/mule-assistant/src/policy.rs` (allowlist et confirmations).*
