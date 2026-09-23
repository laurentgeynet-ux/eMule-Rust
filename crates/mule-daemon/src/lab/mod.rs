// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 EMule-Rust Contributors

pub mod downloader;
pub mod source_service;

pub use downloader::run_lab_download;
pub use source_service::LabSourceService;
