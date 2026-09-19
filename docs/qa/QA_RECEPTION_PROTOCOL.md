# Protocole de Réception QA & Validation Interne — Emule-Sion RC v0.1.0-lab

Ce document définit les vérifications obligatoires à exécuter par tout testeur interne recevant le paquet autonome **Emule-Sion Release Candidate** (`v0.1.0-lab`) sur une machine Windows 10 ou Windows 11 propre.

---

> [!IMPORTANT]
> **Périmètre & Confinement Réseau Stricte**  
> *Emule-Sion* est une application de laboratoire local avec compatibilité eD2K expérimentale.  
> - **Zéro réseau public** : aucune tentative de connexion vers un serveur eD2K externe, aucun nœud Kad, aucun échange P2P sur Internet.  
> - **Loopback exclusif** : toutes les communications inter-processus (IPC WebSocket JSON-RPC et pairs de test locaux) sont confinées à l'adresse `127.0.0.1`.  
> - **Attestation formelle** : *« Compatibilité eD2K expérimentale validée sur corpus statique et pair de référence émulé, en environnement loopback fermé. Le harnais Tier 3 pour un binaire eMule/aMule externe épinglé est disponible, mais son test live reste ignoré par défaut tant qu’un binaire vérifié n’est pas fourni. »*

---

## 1. Vérification Cryptographique de l'Archive

Avant toute décompression, vérifiez l'empreinte SHA-256 du fichier ZIP téléchargé :

```powershell
Get-FileHash .\EmuleSion-v0.1.0-lab-windows-x64.zip -Algorithm SHA256
```

L'empreinte obtenue doit correspondre rigoureusement à la valeur publiée dans le fichier compagnon `.zip.sha256` :

```text
09023bb37bfb45d5a028ada95f5681a4e42df451badd04ca4b9d4ecc99836245
```
*(ou l'empreinte scellée de la Release Candidate correspondante)*

---

## 2. Extraction et Contrôle d'Intégrité des Fichiers

Extrayez l'archive dans un dossier de votre choix (ex: `C:\EmuleSion-RC` ou `%LOCALAPPDATA%\Programs\EmuleSion`) :

```powershell
Expand-Archive -Path .\EmuleSion-v0.1.0-lab-windows-x64.zip -DestinationPath .\
```

Puis exécutez la boucle de vérification cryptographique sur la totalité des fichiers du bundle :

```powershell
Get-Content .\EmuleSion-Windows\SHA256SUMS.txt |
ForEach-Object {
    $parts = $_ -split '\s{2,}', 2
    if ($parts.Count -eq 2) {
        $expected = $parts[0].Trim().ToLowerInvariant()
        $relPath = $parts[1].Trim()
        $path = Join-Path .\EmuleSion-Windows $relPath
        if (!(Test-Path $path)) {
            throw "Fichier manquant dans le bundle : $relPath"
        }
        $actual = (Get-FileHash $path -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($actual -ne $expected) {
            throw "Erreur d'intégrité (hash mismatch) sur : $relPath (attendu: $expected, obtenu: $actual)"
        }
    }
}
Write-Host "✅ Intégrité vérifiée avec succès : 100% des fichiers sont conformes." -ForegroundColor Green
```

---

## 3. Examen du Manifeste et des Licences

1. **Manifeste de version** : ouvrez `.\EmuleSion-Windows\version.json` et vérifiez :
   - `schemaVersion`: `1`
   - `product`: `"Emule-Sion"`
   - `version`: `"0.1.0-lab"`
   - `gitDirty`: `false` (preuve de construction depuis un commit propre)
   - `signed`: `false` (distribution lab non signée Authenticode commerciale)
   - `artifacts`: liste de tous les binaires et fichiers de données avec hachages SHA-256.

2. **Inventaire SBOM & Licences** :
   - `.\EmuleSion-Windows\sbom\sbom.cdx.json` : inventaire CycloneDX 1.5 standardisé des 239 composants.
   - `.\EmuleSion-Windows\sbom\license-audit.md` : rapport de conformité confirmant 100% d'adéquation avec la politique `deny.toml` (licences permissives MIT, Apache-2.0, BSD, ISC, et GPL-2.0-or-later pour les sources dérivées).

---

## 4. Lancement et Validation de l'Interface Utilisateur

### 4.1 Exécution
Lancez l'application principale :
```powershell
.\EmuleSion-Windows\EmuleSion.exe
```

### 4.2 Comportement Attendu
1. L'exécutable `EmuleSion.exe` démarre sans privilège administrateur (compte utilisateur Windows standard).
2. L'interface Flutter Desktop s'affiche instantanément avec le thème sombre eMule moderne.
3. Le démon d'arrière-plan `bin\mule-daemon.exe` est automatiquement initialisé par le superviseur dans le répertoire utilisateur `%APPDATA%\emule_sion` (ou `.mule_data`).
4. L'indicateur de statut en haut à droite passe à **« Connecté au démon local »** (vert).
5. Aucun message d'alerte pare-feu Windows ne doit s'afficher, car le socket écoute exclusivement sur le loopback (`127.0.0.1`).

---

## 5. Matrice de Validation du Superviseur

| Scénario | Action QA | Résultat Attendu |
| :--- | :--- | :--- |
| **Port 4660 occupé** | Lancer une autre application sur le port TCP 4660 avant d'ouvrir Emule-Sion. | Le démon bascule automatiquement sur un port éphémère loopback (`127.0.0.1:XXXX`). L'UI découvre le port via `session.json` et se connecte sans erreur. |
| **Deux instances UI simultanées** | Double-cliquer une 2e fois sur `EmuleSion.exe`. | La seconde instance se connecte au démon déjà actif sans tenter d'en lancer un second (`mule.lock` respecté). |
| **Arrêt inattendu du démon** | Tuer `mule-daemon.exe` depuis le Gestionnaire des tâches. | L'UI affiche l'état « Reconnexion... » avec backoff exponentiel, nettoie le verrou périmé et relance un démon propre. |
| **Dossier avec espaces/accents** | Déplacer le dossier `EmuleSion-Windows` dans `C:\Test Émule Sion 📁\`. | L'application et son démon enfant démarrent et fonctionnent sans erreur de chemin. |
| **Fermeture de l'UI** | Fermer la fenêtre principale d'Emule-Sion. | Le démon enfant s'arrête proprement et `session.json` est automatiquement supprimé. |

---

## 6. Règles de Signalement des Anomalies

Si un écart est constaté lors de ce protocole :
1. Notez le code d'erreur exact ou l'empreinte non concordante.
2. Vérifiez si l'antivirus (ex: Windows Defender) a bloqué l'accès au port local ou au fichier de jeton temporaire.
3. Ne transmettez **aucun fichier de session**, ni journal contenant des chemins personnels ou des noms d'utilisateurs.
