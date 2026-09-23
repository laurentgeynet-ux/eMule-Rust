// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mule_client/mule_client.dart';
import 'package:app_shell/app/app_controller.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:app_shell/app/providers.dart';
import 'package:app_shell/l10n/generated/app_localizations.dart';
import 'package:app_shell/ui/features/shares/views/shares_view.dart';

void main() {
  group('SharesView UI Tests', () {
    late FakeMuleDaemonClient fakeClient;
    late AppController viewModel;

    setUp(() async {
      fakeClient = FakeMuleDaemonClient(enableAutoSimulation: false);
      viewModel = AppController(client: fakeClient);
      await viewModel.initialize(url: Uri.parse('ws://fake:4660/rpc'));
    });

    tearDown(() {
      viewModel.dispose();
    });

    Future<void> pumpShares(WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);
      await viewModel.shares.fetch();
      await tester.pumpWidget(
        ProviderScope(
          overrides: [
            appControllerProvider.overrideWith((ref) => viewModel),
            devToolsEnabledProvider
                .overrideWithValue(viewModel.isDevToolsAvailable),
            daemonSupervisorProvider
                .overrideWithValue(viewModel.supervisor),
            muleDaemonClientProvider.overrideWithValue(viewModel.client),
          ],
          child: MaterialApp(
            locale: const Locale('fr'),
            localizationsDelegates: AppLocalizations.localizationsDelegates,
            supportedLocales: AppLocalizations.supportedLocales,
            home: const Scaffold(body: SharesView()),
          ),
        ),
      );
      await tester.pumpAndSettle();
    }

    testWidgets('delete icon on a complete share removes it after confirm',
        (WidgetTester tester) async {
      await fakeClient.addSharedFile(
        fileHash: 'aabbccddeeff00112233445566778899',
        fileName: 'movie.avi',
        fileSize: 700000000,
        filePath: r'C:\incoming\movie.avi',
      );
      await pumpShares(tester);

      expect(find.text('movie.avi'), findsOneWidget);
      final deleteBtn =
          find.byKey(const Key('delete_share_aabbccddeeff00112233445566778899'));
      expect(deleteBtn, findsOneWidget);

      await tester.tap(deleteBtn);
      await tester.pumpAndSettle();

      // Dialogue de confirmation avant suppression définitive.
      expect(find.text('Supprimer le fichier ?'), findsOneWidget);
      expect(find.textContaining('définitivement supprimé'), findsOneWidget);

      await tester.tap(find.text('Supprimer'));
      await tester.pumpAndSettle();

      expect(find.text('movie.avi'), findsNothing);
      expect(fakeClient.listSharedFiles(), completion(isEmpty));
    });

    testWidgets('rename icon on a complete share renames it via dialog',
        (WidgetTester tester) async {
      await fakeClient.addSharedFile(
        fileHash: 'bbccddee00112233445566778899aabb',
        fileName: 'old_name.iso',
        fileSize: 4000,
        filePath: r'C:\incoming\old_name.iso',
      );
      await pumpShares(tester);

      expect(find.text('old_name.iso'), findsOneWidget);
      final renameBtn = find
          .byKey(const Key('rename_share_bbccddee00112233445566778899aabb'));
      expect(renameBtn, findsOneWidget);

      await tester.tap(renameBtn);
      await tester.pumpAndSettle();

      // Dialogue pré-rempli avec le nom courant (le champ de recherche de
      // la vue existe aussi — on cible le dialogue).
      final field = find.descendant(
        of: find.byType(AlertDialog),
        matching: find.byType(TextField),
      );
      expect(field, findsOneWidget);
      await tester.enterText(field, 'new_name.iso');
      await tester.tap(find.text('Renommer'));
      await tester.pumpAndSettle();

      expect(find.text('new_name.iso'), findsOneWidget);
      expect(find.text('old_name.iso'), findsNothing);
      final files = await fakeClient.listSharedFiles();
      expect(files.single.fileName, 'new_name.iso');
    });

    testWidgets('rename dialog rejects invalid names inline',
        (WidgetTester tester) async {
      await fakeClient.addSharedFile(
        fileHash: 'ccdd11223344556677889900aabbeeff',
        fileName: 'valid.bin',
        fileSize: 100,
        filePath: r'C:\incoming\valid.bin',
      );
      await pumpShares(tester);

      await tester.tap(find
          .byKey(const Key('rename_share_ccdd11223344556677889900aabbeeff')));
      await tester.pumpAndSettle();

      await tester.enterText(
        find.descendant(
          of: find.byType(AlertDialog),
          matching: find.byType(TextField),
        ),
        'bad/name.bin',
      );
      await tester.tap(find.text('Renommer'));
      await tester.pumpAndSettle();

      // Le dialogue reste ouvert avec l'erreur en ligne.
      expect(find.textContaining('invalide'), findsOneWidget);
      expect((await fakeClient.listSharedFiles()).single.fileName,
          'valid.bin');
    });

    testWidgets(
        'search filters shares and groups by category sorted by uploaded total',
        (WidgetTester tester) async {
      final cat = await fakeClient.createCategory(
        name: 'Films',
        color: '#38BDF8',
      );
      await viewModel.categoriesCtl.fetch();
      // Deux fichiers dans la catégorie + un non classé ; l'ordre dans le
      // groupe suit le total uploadé décroissant.
      await fakeClient.addSharedFile(
        fileHash: 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
        fileName: 'less-uploaded.avi',
        fileSize: 100,
        filePath: r'C:\incoming\less-uploaded.avi',
        uploadedBytes: 10,
        categoryId: cat.id,
      );
      await fakeClient.addSharedFile(
        fileHash: 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
        fileName: 'most-uploaded.avi',
        fileSize: 100,
        filePath: r'C:\incoming\most-uploaded.avi',
        uploadedBytes: 5000,
        categoryId: cat.id,
      );
      await fakeClient.addSharedFile(
        fileHash: 'cccccccccccccccccccccccccccccccc',
        fileName: 'uncategorized-file.bin',
        fileSize: 100,
        filePath: r'C:\incoming\uncategorized-file.bin',
      );
      await pumpShares(tester);

      // En-tête de catégorie + groupe « Sans catégorie ».
      expect(find.text('Films'), findsOneWidget);
      expect(find.text('Sans catégorie'), findsOneWidget);

      // Dans « Films », le plus uploadé est rendu en premier.
      final dyMost =
          tester.getTopLeft(find.text('most-uploaded.avi')).dy;
      final dyLess =
          tester.getTopLeft(find.text('less-uploaded.avi')).dy;
      expect(dyMost, lessThan(dyLess));
      // Le groupe « Sans catégorie » arrive après la catégorie nommée.
      final dyUncat =
          tester.getTopLeft(find.text('uncategorized-file.bin')).dy;
      expect(dyUncat, greaterThan(dyLess));

      // Recherche : seul le fichier correspondant reste.
      final search = find.byType(TextField);
      expect(search, findsOneWidget);
      await tester.enterText(search, 'most-uploaded');
      await tester.pumpAndSettle();
      expect(find.text('most-uploaded.avi'), findsOneWidget);
      expect(find.text('less-uploaded.avi'), findsNothing);
      expect(find.text('uncategorized-file.bin'), findsNothing);
    });

    testWidgets('share row shows upload rate and total, refreshed live',
        (WidgetTester tester) async {
      // `emitUploadTick` simule un tick `uploadsUpdated` vers le hash
      // aabb…8899 et accumule le total par fichier (déterministe).
      await fakeClient.addSharedFile(
        fileHash: 'aabbccddeeff00112233445566778899',
        fileName: 'shared-sample.iso',
        fileSize: 700000000,
        filePath: r'C:\incoming\shared-sample.iso',
      );
      await fakeClient.addSharedFile(
        fileHash: 'ffeeddccbbaa00112233445566778899',
        fileName: 'idle.bin',
        fileSize: 1000,
        filePath: r'C:\incoming\idle.bin',
      );
      await pumpShares(tester);

      // Avant le premier tick d'upload : débit et total à zéro.
      expect(find.text('↑ 0 B/s'), findsNWidgets(2));
      expect(find.text('Σ 0 B'), findsNWidgets(2));

      // Tick → uploadsEvents → debounce 1 s → fetch → la ligne affiche
      // le débit mesuré et le total cumulé.
      fakeClient.emitUploadTick();
      // Le debounce du controller est un Timer de la zone réelle (attach
      // en setUp) — on force le refetch plutôt que d'attendre en réel.
      await viewModel.shares.fetch();
      await tester.pump();

      final statsCol = find.byKey(
          const Key('share_upload_stats_aabbccddeeff00112233445566778899'));
      expect(statsCol, findsOneWidget);
      // Débit mesuré > 0 (48–244 KB/s selon le tick simulé).
      expect(
        find.descendant(of: statsCol, matching: find.text('↑ 0 B/s')),
        findsNothing,
      );
      expect(
        find.descendant(of: statsCol, matching: find.textContaining('KB/s')),
        findsOneWidget,
      );
      // Total cumulé > 0.
      expect(
        find.descendant(of: statsCol, matching: find.text('Σ 0 B')),
        findsNothing,
      );

      // Le fichier sans session d'upload reste à zéro.
      final idleCol = find.byKey(
          const Key('share_upload_stats_ffeeddccbbaa00112233445566778899'));
      expect(
        find.descendant(of: idleCol, matching: find.text('↑ 0 B/s')),
        findsOneWidget,
      );
      expect(
        find.descendant(of: idleCol, matching: find.text('Σ 0 B')),
        findsOneWidget,
      );
    });

    test('uploadsEvents triggers a debounced refetch of share stats',
        () async {
      // Hors testWidgets : zone réelle → le debounce (1 s) du controller
      // se déclenche pour de vrai après un tick `uploadsUpdated`.
      await fakeClient.addSharedFile(
        fileHash: 'aabbccddeeff00112233445566778899',
        fileName: 'shared-sample.iso',
        fileSize: 700000000,
        filePath: r'C:\incoming\shared-sample.iso',
      );
      await viewModel.shares.fetch();
      expect(viewModel.shares.files.single.uploadRateBps, 0);

      fakeClient.emitUploadTick();
      await Future.delayed(const Duration(milliseconds: 1400));

      final f = viewModel.shares.files.single;
      expect(f.uploadRateBps, greaterThan(0));
      expect(f.uploadedBytes, greaterThan(0));
    });

    testWidgets('cancel keeps the shared file', (WidgetTester tester) async {
      await fakeClient.addSharedFile(
        fileHash: '11223344556677889900aabbccddeeff',
        fileName: 'kept.iso',
        fileSize: 1000,
        filePath: r'C:\incoming\kept.iso',
      );
      await pumpShares(tester);

      await tester.tap(
          find.byKey(const Key('delete_share_11223344556677889900aabbccddeeff')));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Annuler'));
      await tester.pumpAndSettle();

      expect(find.text('kept.iso'), findsOneWidget);
    });
  });
}
