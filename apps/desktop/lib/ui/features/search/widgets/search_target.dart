/// Search transport target: connected eD2K server, global UDP broadcast to
/// known servers, or Kad DHT keyword index.
enum SearchTarget {
  server,
  global,
  kad;

  String get label {
    switch (this) {
      case SearchTarget.server:
        return 'Serveur eD2K';
      case SearchTarget.global:
        return 'Global (UDP)';
      case SearchTarget.kad:
        return 'Réseau Kad DHT';
    }
  }
}
