// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use serde_json::Value;

use crate::error::JsonRpcError;
use crate::services::DaemonEvent;
use crate::state::AppState;

pub async fn handle_upnp_status(state: &AppState) -> Result<Value, JsonRpcError> {
    let snap = state.upnp.snapshot().await;
    Ok(serde_json::to_value(snap).unwrap_or_default())
}

pub async fn handle_upnp_map_ports(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let snap = state.upnp.snapshot().await;
    let tcp_port = params
        .as_ref()
        .and_then(|p| p["tcpPort"].as_u64())
        .map(|p| p as u16)
        .unwrap_or(snap.tcp_port);
    let udp_port = params
        .as_ref()
        .and_then(|p| p["udpPort"].as_u64())
        .map(|p| p as u16)
        .unwrap_or(snap.udp_port);

    if tcp_port < 1024 || udp_port < 1024 {
        return Err(JsonRpcError::invalid_params(
            "Privileged ports (< 1024) are not allowed for UPnP mapping",
        ));
    }

    let _ = state.upnp.map_ports(tcp_port, udp_port).await;
    let updated_snap = state.upnp.snapshot().await;

    DaemonEvent::UpnpStatusChanged {
        snapshot: updated_snap.clone(),
    }
    .send(&state.event_tx);

    Ok(serde_json::to_value(updated_snap).unwrap_or_default())
}

pub async fn handle_upnp_unmap_ports(state: &AppState) -> Result<Value, JsonRpcError> {
    let _ = state.upnp.unmap_ports().await;
    let snap = state.upnp.snapshot().await;

    DaemonEvent::UpnpStatusChanged {
        snapshot: snap.clone(),
    }
    .send(&state.event_tx);

    Ok(serde_json::to_value(snap).unwrap_or_default())
}

pub async fn handle_upnp_set_enabled(
    state: &AppState,
    params: Option<Value>,
) -> Result<Value, JsonRpcError> {
    let params = params.ok_or_else(|| JsonRpcError::invalid_params("Missing params"))?;
    let enabled = params["enabled"]
        .as_bool()
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'enabled' boolean parameter"))?;

    state.upnp.set_enabled(enabled).await;
    if !enabled {
        let _ = state.upnp.unmap_ports().await;
    }
    let snap = state.upnp.snapshot().await;

    DaemonEvent::UpnpStatusChanged {
        snapshot: snap.clone(),
    }
    .send(&state.event_tx);

    Ok(serde_json::to_value(snap).unwrap_or_default())
}
