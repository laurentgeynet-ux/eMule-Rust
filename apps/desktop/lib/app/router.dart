import 'dart:async';

import 'package:go_router/go_router.dart';

import '../ui/features/downloads/views/downloads_screen.dart';
import '../ui/features/downloads/view_models/downloads_view_model.dart';
import '../ui/features/history/views/history_view.dart';
import '../ui/features/kad/views/kad_screen.dart';
import '../ui/features/search/views/search_screen.dart';
import '../ui/features/servers/views/servers_screen.dart';

/// Canonical route path for each navigation section.
String sectionToPath(AppNavSection section) => switch (section) {
      AppNavSection.downloads => '/downloads',
      AppNavSection.history => '/history',
      AppNavSection.servers => '/servers',
      AppNavSection.search => '/search',
      AppNavSection.kad => '/kad',
    };

/// Inverse of [sectionToPath]. Returns null for unknown paths.
AppNavSection? pathToSection(String path) => switch (path) {
      '/downloads' => AppNavSection.downloads,
      '/history' => AppNavSection.history,
      '/servers' => AppNavSection.servers,
      '/search' => AppNavSection.search,
      '/kad' => AppNavSection.kad,
      _ => null,
    };

/// Declarative router for the app.
///
/// The [DownloadsViewModel] remains the source of truth for the current
/// section (sidebar taps and programmatic [setSection] keep working).
/// Synchronization is one-directional per side, avoiding redirect races:
/// - VM → URL: a listener on the VM calls `router.go()` when the section
///   changes.
/// - URL → VM: the shell adopts the matched location (deep links, browser
///   back/forward) by calling [setSection] after the frame.
GoRouter createAppRouter(DownloadsViewModel viewModel) {
  late final GoRouter router;

  void onSectionChanged() {
    try {
      final wanted = sectionToPath(viewModel.currentSection);
      if (router.state.matchedLocation != wanted) {
        router.go(wanted);
      }
    } catch (_) {
      // Router already disposed (app teardown / test cleanup).
    }
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
          final section = pathToSection(state.matchedLocation);
          if (section != null && section != viewModel.currentSection) {
            // URL-driven navigation: adopt the section into the VM.
            unawaited(
              Future.microtask(() => viewModel.setSection(section)),
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
            path: '/history',
            pageBuilder: (context, state) => NoTransitionPage(
              child: HistoryView(viewModel: viewModel),
            ),
          ),
          GoRoute(
            path: '/servers',
            pageBuilder: (context, state) => NoTransitionPage(
              child: ServersView(viewModel: viewModel),
            ),
          ),
          GoRoute(
            path: '/search',
            pageBuilder: (context, state) => NoTransitionPage(
              child: SearchView(viewModel: viewModel),
            ),
          ),
          GoRoute(
            path: '/kad',
            pageBuilder: (context, state) => NoTransitionPage(
              child: KadView(viewModel: viewModel),
            ),
          ),
        ],
      ),
    ],
  );

  viewModel.addListener(onSectionChanged);
  return router;
}
