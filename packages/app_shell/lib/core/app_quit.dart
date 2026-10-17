// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

/// Coordination entre la fermeture de la fenêtre et la vraie sortie de
/// l'application (comportement « réduire dans le systray » des clients P2P).
///
/// Desktop : `TraySetup` active [closeToTray] une fois l'icône systray et
/// l'interception de fermeture (`setPreventClose`) en place. Une demande de
/// fermeture ne fait alors que masquer la fenêtre — le handler d'exit de
/// `eMuleRustApp` l'annule pour que le daemon continue de tourner.
///
/// La sortie explicite (menu systray « Quitter ») positionne [quitting]
/// avant de redéclencher une fermeture ; le handler arrête alors le daemon
/// et laisse le processus se terminer.
///
/// Dart pur (pas de `window_manager`) : ce fichier vit dans `app_shell`,
/// qui ne dépend pas des plugins desktop.
class AppQuit {
  AppQuit._();

  /// Mode close-to-tray actif : les demandes de fermeture masquent la
  /// fenêtre au lieu de quitter.
  static bool closeToTray = false;

  /// Une vraie sortie est en cours — le handler d'exit doit arrêter le
  /// daemon puis laisser l'application se terminer.
  static bool quitting = false;

  /// Action de « masquer dans le systray », appelée par le handler d'exit
  /// quand une fermeture est annulée. Enregistrée par `TraySetup` (desktop)
  /// — `app_shell` ne dépend pas de `window_manager`.
  ///
  /// Nécessaire car le moteur Flutter consomme `WM_CLOSE` pour son mécanisme
  /// d'exit-request : avec une réponse `cancel`, l'événement `close` n'atteint
  /// pas les listeners `window_manager`, donc le masquage doit partir d'ici.
  static Future<void> Function()? onHideToTray;
}
