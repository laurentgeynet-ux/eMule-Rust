import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mule_client/mule_client.dart';
import 'package:desktop/main.dart';
import 'package:desktop/app/app_controller.dart';
import 'package:desktop/ui/features/downloads/widgets/category_edit_dialog.dart';
import 'package:desktop/ui/features/downloads/widgets/downloads_sidebar.dart';

void main() {
  group('Category Management Widget Tests', () {
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

    testWidgets('displays categories in sidebar with counts and virtual entries', (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(EmuleSionApp(viewModel: viewModel));
      await tester.pumpAndSettle();

      // Verify Categories section in sidebar
      expect(find.text('CATEGORIES'), findsOneWidget);
      expect(find.text('All Downloads'), findsAtLeastNWidgets(1));
      expect(find.text('Sans catégorie'), findsOneWidget);
      expect(
        find.descendant(of: find.byType(DownloadsSidebar), matching: find.text('Linux ISOs')),
        findsOneWidget,
      );
      expect(
        find.descendant(of: find.byType(DownloadsSidebar), matching: find.text('Media & 3D')),
        findsOneWidget,
      );

      // Verify initial downloads are all present
      expect(find.text('Debian_12_netinst.iso'), findsOneWidget);
      expect(find.text('Blender_Benchmark_Scene.blend'), findsOneWidget);
      expect(find.text('Linux_Kernel_6.10.tar.xz'), findsOneWidget);
    });

    testWidgets('filters downloads when a category is selected in sidebar', (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(EmuleSionApp(viewModel: viewModel));
      await tester.pumpAndSettle();

      // Click on 'Linux ISOs' in sidebar
      await tester.tap(
        find.descendant(of: find.byType(DownloadsSidebar), matching: find.text('Linux ISOs')),
      );
      await tester.pumpAndSettle();

      // Only Debian should be visible
      expect(find.text('Debian_12_netinst.iso'), findsOneWidget);
      expect(find.text('Blender_Benchmark_Scene.blend'), findsNothing);
      expect(find.text('Linux_Kernel_6.10.tar.xz'), findsNothing);

      // Click on 'Media & 3D' in sidebar
      await tester.tap(
        find.descendant(of: find.byType(DownloadsSidebar), matching: find.text('Media & 3D')),
      );
      await tester.pumpAndSettle();

      // Only Blender should be visible
      expect(find.text('Blender_Benchmark_Scene.blend'), findsOneWidget);
      expect(find.text('Debian_12_netinst.iso'), findsNothing);
      expect(find.text('Linux_Kernel_6.10.tar.xz'), findsNothing);

      // Click on 'Sans catégorie' in sidebar
      await tester.tap(
        find.descendant(of: find.byType(DownloadsSidebar), matching: find.text('Sans catégorie')),
      );
      await tester.pumpAndSettle();

      // Only Linux Kernel should be visible
      expect(find.text('Linux_Kernel_6.10.tar.xz'), findsOneWidget);
      expect(find.text('Debian_12_netinst.iso'), findsNothing);
      expect(find.text('Blender_Benchmark_Scene.blend'), findsNothing);

      // Reset category filter by clicking 'All Downloads' (folder icon)
      await tester.tap(find.byIcon(Icons.folder_outlined));
      await tester.pumpAndSettle();

      expect(find.text('Debian_12_netinst.iso'), findsOneWidget);
      expect(find.text('Blender_Benchmark_Scene.blend'), findsOneWidget);
      expect(find.text('Linux_Kernel_6.10.tar.xz'), findsOneWidget);
    });

    testWidgets('opens CategoryEditDialog and creates a new category', (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(EmuleSionApp(viewModel: viewModel));
      await tester.pumpAndSettle();

      // Click Add Category button (+) in sidebar
      final addCategoryBtn = find.byTooltip('Add Category');
      expect(addCategoryBtn, findsOneWidget);
      await tester.tap(addCategoryBtn);
      await tester.pumpAndSettle();

      // Verify dialog is opened
      expect(find.byType(CategoryEditDialog), findsOneWidget);
      expect(find.text('New Category'), findsOneWidget);

      // Enter category name in the dialog TextField
      final nameField = find.descendant(
        of: find.byType(CategoryEditDialog),
        matching: find.byType(TextField),
      ).first;
      await tester.enterText(nameField, 'Archives');
      await tester.pumpAndSettle();

      // Submit form
      final createBtn = find.text('Create Category');
      await tester.tap(createBtn);
      await tester.pumpAndSettle();

      // Verify dialog closed and new category is visible in sidebar
      expect(find.byType(CategoryEditDialog), findsNothing);
      expect(find.text('Archives'), findsOneWidget);
      expect(viewModel.categories.any((c) => c.name == 'Archives'), isTrue);
    });

    testWidgets('assigns download category and updates category count', (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(EmuleSionApp(viewModel: viewModel));
      await tester.pumpAndSettle();

      // Initially, Linux Kernel has no category
      final kernelDownload = viewModel.filteredDownloads.firstWhere((d) => d.fileName.contains('Kernel'));
      expect(kernelDownload.categoryId, isNull);

      final isoCat = viewModel.categories.firstWhere((c) => c.name.contains('ISOs'));

      // Assign kernel to ISO category via viewModel
      await viewModel.setDownloadCategory(kernelDownload.id, isoCat.id);
      await tester.pumpAndSettle();

      final updatedKernel = viewModel.filteredDownloads.firstWhere((d) => d.id == kernelDownload.id);
      expect(updatedKernel.categoryId, equals(isoCat.id));
      expect(viewModel.categoryCounts[isoCat.id], equals(2));
      expect(viewModel.uncategorizedCount, equals(0));
    });

    testWidgets('deleting category preserves downloads and moves them to uncategorized', (WidgetTester tester) async {
      tester.view.physicalSize = const Size(1280, 800);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.resetPhysicalSize);

      await tester.pumpWidget(EmuleSionApp(viewModel: viewModel));
      await tester.pumpAndSettle();

      final isoCat = viewModel.categories.firstWhere((c) => c.name.contains('ISOs'));
      final debian = viewModel.filteredDownloads.firstWhere((d) => d.fileName.contains('Debian'));
      expect(debian.categoryId, equals(isoCat.id));

      // Delete ISO category
      await viewModel.deleteCategory(isoCat.id);
      await tester.pumpAndSettle();

      // Category should be gone
      expect(viewModel.categories.any((c) => c.id == isoCat.id), isFalse);

      // Crucial invariant: Debian download is NOT deleted!
      final preservedDebian = viewModel.filteredDownloads.firstWhere((d) => d.id == debian.id);
      expect(preservedDebian.categoryId, isNull);
      expect(viewModel.uncategorizedCount, equals(2)); // Debian + Linux Kernel
      expect(find.text('Debian_12_netinst.iso'), findsOneWidget);
    });
  });
}
