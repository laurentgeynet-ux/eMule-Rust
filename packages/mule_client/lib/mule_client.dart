// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

library mule_client;

// Domain models
export 'domain/models/category_item.dart';
export 'domain/models/daemon_event.dart';
export 'domain/models/daemon_info.dart';
export 'domain/models/download_item.dart';
export 'domain/models/filter_models.dart';
export 'domain/models/history_entry.dart';
export 'domain/models/history_stats.dart';
export 'domain/models/lab_models.dart';
export 'domain/models/network_policy_models.dart';
export 'domain/models/part_met_models.dart';
export 'domain/models/server_models.dart';

// Client interface
export 'domain/services/mule_daemon_client.dart';

// Implementations + DTOs
export 'data/rpc_daemon_client.dart';
export 'data/fake_daemon_client.dart';
export 'data/dto/rpc_dto.dart';
