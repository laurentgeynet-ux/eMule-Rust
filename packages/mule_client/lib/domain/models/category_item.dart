// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

import 'package:flutter/material.dart';

@immutable
class CategoryItem {
  final String id;
  final String name;
  final String color; // Hex "#RRGGBB"
  final int sortOrder;
  final String? defaultDestination;
  final String? defaultPriority; // 'low', 'normal', 'high'
  final DateTime createdAt;
  final DateTime updatedAt;

  const CategoryItem({
    required this.id,
    required this.name,
    required this.color,
    this.sortOrder = 0,
    this.defaultDestination,
    this.defaultPriority,
    required this.createdAt,
    required this.updatedAt,
  });

  Color get parsedColor {
    final hex = color.replaceAll('#', '').trim();
    if (hex.length == 6) {
      final val = int.tryParse('FF$hex', radix: 16);
      if (val != null) return Color(val);
    } else if (hex.length == 8) {
      final val = int.tryParse(hex, radix: 16);
      if (val != null) return Color(val);
    }
    return const Color(0xFF2196F3);
  }

  CategoryItem copyWith({
    String? id,
    String? name,
    String? color,
    int? sortOrder,
    String? defaultDestination,
    String? defaultPriority,
    DateTime? createdAt,
    DateTime? updatedAt,
  }) {
    return CategoryItem(
      id: id ?? this.id,
      name: name ?? this.name,
      color: color ?? this.color,
      sortOrder: sortOrder ?? this.sortOrder,
      defaultDestination: defaultDestination ?? this.defaultDestination,
      defaultPriority: defaultPriority ?? this.defaultPriority,
      createdAt: createdAt ?? this.createdAt,
      updatedAt: updatedAt ?? this.updatedAt,
    );
  }

  @override
  bool operator ==(Object other) =>
      identical(this, other) ||
      other is CategoryItem &&
          runtimeType == other.runtimeType &&
          id == other.id &&
          name == other.name &&
          color == other.color &&
          sortOrder == other.sortOrder &&
          defaultDestination == other.defaultDestination &&
          defaultPriority == other.defaultPriority &&
          updatedAt == other.updatedAt;

  @override
  int get hashCode =>
      id.hashCode ^
      name.hashCode ^
      color.hashCode ^
      sortOrder.hashCode ^
      (defaultDestination?.hashCode ?? 0) ^
      (defaultPriority?.hashCode ?? 0) ^
      updatedAt.hashCode;
}
