/// Platform abstraction for OS-level file actions (open folder, reveal file).
/// Stub implementation used on web: local filesystem doesn't exist.
class SystemActions {
  const SystemActions();

  /// Whether the platform can open local directories/files.
  bool get isSupported => false;

  /// Path separator for display purposes.
  String get pathSeparator => '/';

  /// Ensures [path] exists (if creatable) and opens it in the OS file
  /// manager. No-op on unsupported platforms.
  Future<void> openDirectory(String path) async {}

  /// Reveals [filePath] in the OS file manager, falling back to opening its
  /// parent directory. No-op on unsupported platforms.
  Future<void> revealFile(String filePath) async {}
}

/// Shared instance — resolved per-platform via conditional export.
const systemActions = SystemActions();
