import 'dart:async';

import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';

import '../ui/features/downloads/views/downloads_screen.dart';
import '../ui/features/downloads/widgets/settings_dialog.dart';
import 'app_controller.dart';
import '../ui/features/history/views/history_view.dart';
import '../ui/features/kad/views/kad_screen.dart';
import '../ui/features/uploads/views/uploads_view.dart';
import '../ui/features/search/views/search_screen.dart';
import '../ui/features/servers/views/servers_screen.dart';

/// Canonical route path for each navigation section.
String sectionToPath(AppNavSection section) => switch (section) {
      AppNavSection.downloads => '/downloads',
      AppNavSection.uploads => '/uploads',
      AppNavSection.history => '/history',
      AppNavSection.servers => '/servers',
      AppNavSection.search => '/search',
      AppNavSection.kad => '/kad',
    };

/// Inverse of [sectionToPath]. Returns null for unknown paths.
///
/// `/downloads/<id>` and `/settings` resolve to the downloads section: the
/// former selects a download, the latter opens the settings dialog on top
/// of it (handled in the shell builder).
AppNavSection? pathToSection(String path) {
  if (path == '/settings' || path.startsWith('/downloads/')) {
    return AppNavSection.downloads;
  }
  return switch (path) {
    '/downloads' => AppNavSection.downloads,
    '/uploads' => AppNavSection.uploads,
    '/history' => AppNavSection.history,
    '/servers' => AppNavSection.servers,
    '/search' => AppNavSection.search,
    '/kad' => AppNavSection.kad,
    _ => null,
  };
}

/// Declarative router for the app.
///
/// The [AppController] remains the source of truth for the current
/// section (sidebar taps and programmatic [setSection] keep working).
/// Synchronization is one-directional per side, avoiding redirect races:
/// - VM → URL: a listener on the VM calls `router.go()` when the section
///   changes.
/// - URL → VM: the shell adopts the matched location (deep links, browser
///   back/forward) by calling [setSection] after the frame.
GoRouter createAppRouter(AppController viewModel) {
  late final GoRouter router;

  var settingsDialogOpen = false;

  void onSectionChanged() {
    try {
      final wanted = sectionToPath(viewModel.currentSection);
      final location = router.state.matchedLocation;
      // Keep deep links stable while the downloads section stays active.
      if (wanted == '/downloads' && location.startsWith('/downloads/')) {
        return;
      }
      if (location != wanted) {
        router.go(wanted);
      }
    } catch (_) {
      // Router already disposed (app teardown / test cleanup).
    }
  }

  void openSettingsDialog(BuildContext context) {
    settingsDialogOpen = true;
    unawaited(() async {
      if (!context.mounted) {
        settingsDialogOpen = false;
        return;
      }
      await showDialog<void>(
        context: context,
        builder: (_) => SettingsDialog(viewModel: viewModel),
      );
      settingsDialogOpen = false;
      if (router.state.matchedLocation == '/settings') {
        router.go('/downloads');
      }
    }());
  }

  router = GoRouter(
    initialLocation: '/downloads',
    redirect: (context, state) {
      final location = state.matchedLocation;
      if (location == '/' || pathToSection(location) == null) {
        return sectionToPath(viewModel.currentSection);
      }
      return null;
    },
    routes: [
      ShellRoute(
        builder: (context, state, child) {
          final location = state.matchedLocation;
          final section = pathToSection(location);
          if (section != null && section != viewModel.currentSection) {
            // URL-driven navigation: adopt the section into the VM.
            unawaited(
              Future.microtask(() => viewModel.setSection(section)),
            );
          }
          final id = state.pathParameters['id'];
          if (id != null && id != viewModel.selectedDownloadId) {
            unawaited(
              Future.microtask(() => viewModel.selectDownload(id)),
            );
          } else if (location == '/downloads' &&
              viewModel.selectedDownloadId != null) {
            unawaited(
              Future.microtask(() => viewModel.selectDownload(null)),
            );
          }
          if (location == '/settings' && !settingsDialogOpen) {
            unawaited(
              Future.microtask(() {
                if (context.mounted) openSettingsDialog(context);
              }),
            );
          }
          return DownloadsScreen(viewModel: viewModel, child: child);
        },
        routes: [
          GoRoute(
            path: '/downloads',
            pageBuilder: (context, state) => NoTransitionPage(
              child: DownloadsView(viewModel: viewModel),
            ),
          ),
          GoRoute(
            path: '/downloads/:id',
            pageBuilder: (context, state) => NoTransitionPage(
              child: DownloadsView(viewModel: viewModel),
            ),
          ),
          GoRoute(
            path: '/settings',
            pageBuilder: (context, state) => NoTransitionPage(
              child: DownloadsView(viewModel: viewModel),
            ),
          ),
          GoRoute(
            path: '/uploads',
            pageBuilder: (context, state) => NoTransitionPage(
              child: const UploadsView(),
            ),
          ),
          GoRoute(
            path: '/history',
            pageBuilder: (context, state) => NoTransitionPage(
              child: const HistoryView(),
            ),
          ),
          GoRoute(
            path: '/servers',
            pageBuilder: (context, state) => NoTransitionPage(
              child: const ServersView(),
            ),
          ),
          GoRoute(
            path: '/search',
            pageBuilder: (context, state) => NoTransitionPage(
              child: const SearchView(),
            ),
          ),
          GoRoute(
            path: '/kad',
            pageBuilder: (context, state) => NoTransitionPage(
              child: const KadView(),
            ),
          ),
        ],
      ),
    ],
  );

  viewModel.addListener(onSectionChanged);
  return router;
}
