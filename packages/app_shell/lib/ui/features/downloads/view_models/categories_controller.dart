// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet (Loulach)

import 'package:flutter/foundation.dart';
import 'package:mule_client/mule_client.dart';

import '../../../../l10n/l10n_lookup.dart';

/// Sub-domain controller for download categories.
///
/// Owns the category list and the selected sidebar category. Status and
/// error reporting is delegated to the host via [onStatus]/[onError];
/// [onRefreshDownloads] is invoked when a category change requires the
/// download list to be refreshed, and [onDownloadUpdated] pushes an updated
/// item back to the downloads store.
class CategoriesController extends ChangeNotifier {
  final MuleDaemonClient client;

  final void Function(String message)? onStatus;
  final void Function(String message)? onError;

  /// Refresh the downloads list (e.g. after deleting a category).
  final Future<void> Function()? onRefreshDownloads;

  /// Merge an updated download into the downloads store.
  final void Function(DownloadItem item)? onDownloadUpdated;

  CategoriesController({
    required this.client,
    this.onStatus,
    this.onError,
    this.onRefreshDownloads,
    this.onDownloadUpdated,
  });

  List<CategoryItem> _categories = [];
  String? _selectedId;

  List<CategoryItem> get categories => _categories;
  String? get selectedId => _selectedId;

  void select(String? categoryId) {
    _selectedId = categoryId;
    notifyListeners();
  }

  Future<void> fetch() async {
    try {
      _categories = await client.listCategories();
      notifyListeners();
    } catch (e) {
      debugPrint('Failed to fetch categories: $e');
    }
  }

  Future<bool> create({
    required String name,
    required String color,
    String? defaultDestination,
    String? defaultPriority,
  }) async {
    try {
      final cat = await client.createCategory(
        name: name,
        color: color,
        defaultDestination: defaultDestination,
        defaultPriority: defaultPriority,
      );
      await fetch();
      onStatus?.call(L10nLookup.current.msgCategoryCreated(cat.name));
      notifyListeners();
      return true;
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrCategoryCreate(e));
      notifyListeners();
      return false;
    }
  }

  Future<bool> update(
    String id, {
    String? name,
    String? color,
    String? defaultDestination,
    String? defaultPriority,
    int? sortOrder,
  }) async {
    try {
      final cat = await client.updateCategory(
        id,
        name: name,
        color: color,
        defaultDestination: defaultDestination,
        defaultPriority: defaultPriority,
        sortOrder: sortOrder,
      );
      await fetch();
      onStatus?.call(L10nLookup.current.msgCategoryUpdated(cat.name));
      notifyListeners();
      return true;
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrCategoryUpdate(e));
      notifyListeners();
      return false;
    }
  }

  Future<bool> delete(String id) async {
    try {
      final cat = _categories.where((c) => c.id == id).firstOrNull;
      await client.deleteCategory(id);
      if (_selectedId == id) {
        _selectedId = null;
      }
      await onRefreshDownloads?.call();
      onStatus?.call(
        L10nLookup.current.msgCategoryDeleted(cat?.name ?? id),
      );
      notifyListeners();
      return true;
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrCategoryDelete(e));
      notifyListeners();
      return false;
    }
  }

  Future<bool> reorder(List<String> categoryIds) async {
    try {
      await client.reorderCategories(categoryIds);
      await fetch();
      return true;
    } catch (e) {
      onError?.call(L10nLookup.current.msgErrCategoryReorder(e));
      notifyListeners();
      return false;
    }
  }

  Future<bool> assignDownload(
    String downloadId,
    String? categoryId,
  ) async {
    try {
      final updated = await client.setDownloadCategory(
        downloadId,
        categoryId,
      );
      onDownloadUpdated?.call(updated);
      final catName = categoryId != null
          ? (_categories.where((c) => c.id == categoryId).firstOrNull?.name ??
                L10nLookup.current.categoryFallback)
          : L10nLookup.current.none;
      onStatus?.call(L10nLookup.current.msgAssignedToCategory(catName));
      notifyListeners();
      return true;
    } catch (e) {
      onError?.call(L10nLookup.current.msgCatSetError(e));
      notifyListeners();
      return false;
    }
  }
}
