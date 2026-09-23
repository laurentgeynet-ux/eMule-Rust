// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

import 'dart:async';

import 'package:app_links/app_links.dart';
import 'package:flutter/foundation.dart';

import 'package:app_shell/app/app_controller.dart';

/// Receives OS deep links (`ed2k://`) and forwards them to the daemon via
/// [AppController.addEd2kLink]. No-op on web.
///
/// Platform registration (OS-level, done outside this class):
/// - Windows: apply `windows/ed2k_protocol.reg` or register `HKCR\ed2k`
///   from the installer (see docs).
/// - Linux: `.desktop` file declaring `MimeType=x-scheme-handler/ed2k`.
/// - Android/iOS: intent-filter / CFBundleURLTypes entries for `ed2k`.
class DeepLinkHandler {
  final AppController _controller;
  StreamSubscription<Uri>? _sub;

  DeepLinkHandler(this._controller);

  void init() {
    if (kIsWeb) return;
    final links = AppLinks();
    _sub = links.uriLinkStream.listen(
      _onUri,
      onError: (Object _) {},
    );
    unawaited(
      links.getInitialLink().then((uri) {
        if (uri != null) _onUri(uri);
      }),
    );
  }

  void _onUri(Uri uri) {
    final raw = uri.toString();
    if (!raw.startsWith('ed2k://')) return;
    unawaited(_controller.addEd2kLink(raw));
  }

  Future<void> dispose() async => _sub?.cancel();
}
