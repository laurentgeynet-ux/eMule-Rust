// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

//! Tier 3 External Reference Orchestration module.
//!
//! Exposes environment management, binary validation, process guard, and divergence reporting
//! for executing pinned external binaries against eMuleRust.

pub mod environment;
pub mod harness;
pub mod report;

#[allow(unused_imports)]
pub use environment::{ReferenceKind, Tier3Environment};
#[allow(unused_imports)]
pub use harness::{Tier3BinaryValidator, Tier3Harness, Tier3HarnessError, Tier3ProcessGuard};
#[allow(unused_imports)]
pub use report::{DivergenceEntry, ObservedTagReport, Tier3DivergenceReport, WireTraceRecord};
