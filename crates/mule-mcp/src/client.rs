use futures_util::{SinkExt, StreamExt};
use mule_daemon::{DaemonSession, WorkDirectoryResolver};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, oneshot, Mutex};
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, error, info, warn};

type PendingRequests = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, String>>>>>;

#[derive(Clone)]
pub struct DaemonClient {
    _endpoint: String,
    _token: String,
    next_id: Arc<AtomicU64>,
    write_tx: mpsc::Sender<Message>,
    pending: PendingRequests,
    search_events_tx: broadcast::Sender<Value>,
}

impl std::fmt::Debug for DaemonClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DaemonClient")
            .field("endpoint", &self._endpoint)
            .finish()
    }
}

impl DaemonClient {
    /// Discovers and connects to a running mule-daemon instance.
    pub async fn connect(
        explicit_url: Option<String>,
        explicit_token: Option<String>,
        explicit_workdir: Option<PathBuf>,
        explicit_app: Option<PathBuf>,
    ) -> Result<Self, String> {
        let (endpoint, token) =
            Self::resolve_connection(explicit_url, explicit_token, explicit_workdir, explicit_app)
                .await?;
        info!("Connecting to Emule-Sion daemon at {}", endpoint);

        let (ws_stream, _) = tokio_tungstenite::connect_async(&endpoint)
            .await
            .map_err(|e| {
                format!("Failed to connect to daemon at {endpoint}: {e}. Is Emule-Sion running?")
            })?;

        let (mut ws_write, mut ws_read) = ws_stream.split();
        let (write_tx, mut write_rx) = mpsc::channel::<Message>(64);
        let pending = Arc::new(Mutex::new(HashMap::<
            u64,
            oneshot::Sender<Result<Value, String>>,
        >::new()));
        let (search_events_tx, _) = broadcast::channel::<Value>(128);

        // Background task to send messages to WebSocket
        tokio::spawn(async move {
            while let Some(msg) = write_rx.recv().await {
                if let Err(e) = ws_write.send(msg).await {
                    error!("WebSocket write error: {e}");
                    break;
                }
            }
        });

        // Background task to read messages from WebSocket
        let pending_clone = pending.clone();
        let search_tx_clone = search_events_tx.clone();
        tokio::spawn(async move {
            while let Some(msg_res) = ws_read.next().await {
                match msg_res {
                    Ok(Message::Text(text)) => {
                        if let Ok(val) = serde_json::from_str::<Value>(&text) {
                            if let Some(id_u64) = val.get("id").and_then(|i| i.as_u64()) {
                                let mut map = pending_clone.lock().await;
                                if let Some(sender) = map.remove(&id_u64) {
                                    if let Some(err) = val.get("error") {
                                        let err_msg = err
                                            .get("message")
                                            .and_then(|m| m.as_str())
                                            .unwrap_or("RPC error");
                                        let _ = sender.send(Err(err_msg.to_string()));
                                    } else {
                                        let res = val.get("result").cloned().unwrap_or(Value::Null);
                                        let _ = sender.send(Ok(res));
                                    }
                                }
                            } else if let Some(method) = val.get("method").and_then(|m| m.as_str())
                            {
                                debug!("Received daemon notification: {method}");
                                if method == "server.searchResult" {
                                    if let Some(params) = val.get("params") {
                                        let _ = search_tx_clone.send(params.clone());
                                    }
                                }
                            }
                        }
                    }
                    Ok(Message::Close(_)) => {
                        warn!("WebSocket connection closed by daemon");
                        break;
                    }
                    Err(e) => {
                        error!("WebSocket read error: {e}");
                        break;
                    }
                    _ => {}
                }
            }
        });

        let client = Self {
            _endpoint: endpoint,
            _token: token.clone(),
            next_id: Arc::new(AtomicU64::new(1)),
            write_tx,
            pending,
            search_events_tx,
        };

        // Handshake: system.hello
        let hello_params = json!({
            "protocolVersion": 1,
            "token": token
        });
        client
            .call("system.hello", hello_params)
            .await
            .map_err(|e| format!("Handshake with Emule-Sion daemon failed: {e}"))?;

        info!("Successfully connected and authenticated with Emule-Sion daemon");
        Ok(client)
    }

    async fn resolve_connection(
        explicit_url: Option<String>,
        explicit_token: Option<String>,
        explicit_workdir: Option<PathBuf>,
        explicit_app: Option<PathBuf>,
    ) -> Result<(String, String), String> {
        if let Some(url) = explicit_url {
            let token = explicit_token.unwrap_or_default();
            return Ok((url, token));
        }

        // 1. Resolve work directory
        let workdir = if let Some(app_path) = explicit_app {
            let bundle_root = if app_path.is_file() {
                app_path.parent().unwrap_or(&app_path).to_path_buf()
            } else {
                app_path
            };

            if bundle_root.join("portable.flag").exists() || bundle_root.join("data").is_dir() {
                bundle_root.join("data")
            } else {
                bundle_root
            }
        } else if let Some(dir) = explicit_workdir {
            if dir.join("portable.flag").exists() && dir.join("data").is_dir() {
                dir.join("data")
            } else {
                dir
            }
        } else {
            let resolved = WorkDirectoryResolver::resolve(None, None);
            resolved.workdir
        };

        let session = DaemonSession::read(&workdir).map_err(|e| {
            format!(
                "No active Emule-Sion daemon session found in {} ({e}). Please start EmuleSion.",
                workdir.display()
            )
        })?;

        let token = explicit_token
            .or_else(|| DaemonSession::read_token(&workdir))
            .unwrap_or_default();

        Ok((session.endpoint, token))
    }

    pub async fn call(&self, method: &str, params: Value) -> Result<Value, String> {
        // Iterative Kad lookups and multi-node queries can legitimately run
        // longer than the default timeout on a real WAN.
        let timeout_secs = if method.starts_with("kad.search") || method.starts_with("kad.publish")
        {
            60
        } else {
            15
        };
        self.call_with_timeout(method, params, timeout_secs).await
    }

    pub async fn call_with_timeout(
        &self,
        method: &str,
        params: Value,
        timeout_secs: u64,
    ) -> Result<Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let req = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });

        let (tx, rx) = oneshot::channel();
        {
            let mut map = self.pending.lock().await;
            map.insert(id, tx);
        }

        let req_str = serde_json::to_string(&req).map_err(|e| e.to_string())?;
        self.write_tx
            .send(Message::Text(req_str.into()))
            .await
            .map_err(|e| format!("Failed to send RPC request: {e}"))?;

        match tokio::time::timeout(tokio::time::Duration::from_secs(timeout_secs), rx).await {
            Ok(Ok(res)) => res,
            Ok(Err(_)) => Err("RPC response channel dropped unexpectedly".to_string()),
            Err(_) => {
                let mut map = self.pending.lock().await;
                map.remove(&id);
                Err(format!(
                    "RPC call to '{method}' timed out after {timeout_secs} seconds"
                ))
            }
        }
    }

    /// Triggers search on Kad and/or server and collects results.
    pub async fn search_and_collect(
        &self,
        term: &str,
        network: Option<&str>,
        min_size: Option<u64>,
        max_size: Option<u64>,
        extension: Option<&str>,
        wait_seconds: u64,
    ) -> Result<Value, String> {
        let net = network.unwrap_or("kad");
        // Le schéma MCP expose "both" ; "all" reste accepté pour
        // compatibilité avec les clients existants.
        let search_kad = matches!(net, "kad" | "all" | "both");
        let search_server = matches!(net, "server" | "all" | "both");

        let mut collected_files: Vec<Value> = Vec::new();
        let mut seen_hashes = std::collections::HashSet::new();

        // 1. Kad Search
        if search_kad {
            let mut kad_params = json!({ "term": term });
            if let Some(min) = min_size {
                kad_params["minSize"] = json!(min);
            }
            if let Some(max) = max_size {
                kad_params["maxSize"] = json!(max);
            }

            match self.call("kad.searchKeywords", kad_params).await {
                Ok(kad_res) => {
                    if let Some(results) = kad_res.get("results").and_then(|r| r.as_array()) {
                        for item in results {
                            let mut file_obj = item.clone();
                            file_obj["network"] = json!("kad");
                            let name_opt = file_obj
                                .get("fileName")
                                .and_then(|n| n.as_str())
                                .map(String::from);
                            let size_opt = file_obj.get("fileSize").and_then(|s| s.as_u64());
                            let hash_opt = file_obj
                                .get("fileHash")
                                .and_then(|h| h.as_str())
                                .map(String::from);

                            if let (Some(name), Some(size), Some(hash)) =
                                (name_opt, size_opt, hash_opt)
                            {
                                if let Some(ext) = extension {
                                    let ext_with_dot =
                                        format!(".{}", ext.trim_start_matches('.').to_lowercase());
                                    if !name.to_lowercase().ends_with(&ext_with_dot) {
                                        continue;
                                    }
                                }
                                file_obj["ed2kLink"] =
                                    json!(format!("ed2k://|file|{name}|{size}|{hash}|/"));
                                if seen_hashes.insert(hash.to_lowercase()) {
                                    collected_files.push(file_obj);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("Kad searchKeywords error: {e}");
                }
            }
        }

        // 2. Server Search
        if search_server {
            let mut rx = self.search_events_tx.subscribe();

            let mut params = json!({ "term": term });
            if let Some(min) = min_size {
                params["minSize"] = json!(min);
            }
            if let Some(max) = max_size {
                params["maxSize"] = json!(max);
            }
            if let Some(ext) = extension {
                params["extension"] = json!(ext);
            }

            let _ = self.call("servers.search", params).await;

            let deadline =
                tokio::time::Instant::now() + tokio::time::Duration::from_secs(wait_seconds);
            while tokio::time::Instant::now() < deadline {
                let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
                if let Ok(Ok(event_params)) = tokio::time::timeout(remaining, rx.recv()).await {
                    if let Some(files) = event_params.get("files").and_then(|f| f.as_array()) {
                        for f in files {
                            let mut file_obj = f.clone();
                            file_obj["network"] = json!("server");
                            let name_opt = file_obj
                                .get("fileName")
                                .and_then(|n| n.as_str())
                                .map(String::from);
                            let size_opt = file_obj.get("fileSize").and_then(|s| s.as_u64());
                            let hash_opt = file_obj
                                .get("fileHash")
                                .and_then(|h| h.as_str())
                                .map(String::from);

                            if let (Some(name), Some(size), Some(hash)) =
                                (name_opt, size_opt, hash_opt)
                            {
                                file_obj["ed2kLink"] =
                                    json!(format!("ed2k://|file|{name}|{size}|{hash}|/"));
                                if seen_hashes.insert(hash.to_lowercase()) {
                                    collected_files.push(file_obj);
                                }
                            }
                        }
                    }
                } else {
                    break;
                }
            }
        }

        // Sort by sourceCount descending
        collected_files.sort_by(|a, b| {
            let sa = a.get("sourceCount").and_then(|s| s.as_u64()).unwrap_or(0);
            let sb = b.get("sourceCount").and_then(|s| s.as_u64()).unwrap_or(0);
            sb.cmp(&sa)
        });

        Ok(json!({
            "query": term,
            "network": net,
            "resultsCount": collected_files.len(),
            "files": collected_files
        }))
    }
}
