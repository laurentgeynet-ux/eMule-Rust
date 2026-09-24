// SPDX-License-Identifier: GPL-2.0-or-later
// Copyright (C) 2026 Laurent Geynet "Loulach"

use futures_util::StreamExt;
use serde_json::json;

use mule_daemon::WorkDirMode;
use mule_test_support::{recv_response_by_id, send_json, spawn_test_daemon_with_mode};
use tokio_tungstenite::connect_async;

#[tokio::test]
async fn test_category_rpc_lifecycle_and_invariants() {
    let (addr, token, _dir) =
        spawn_test_daemon_with_mode("cat_test_token_789", WorkDirMode::Portable).await;
    let ws_url = format!("ws://{addr}/rpc");
    let (ws_stream, _) = connect_async(&ws_url).await.expect("Failed to connect");
    let (mut write, mut read) = ws_stream.split();

    // 1. Handshake and verify "categories" capability
    send_json(
        &mut write,
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "system.hello",
            "params": {
                "clientName": "TestRunner",
                "clientVersion": "1.0.0",
                "protocolVersion": 1,
                "token": token
            }
        }),
    )
    .await;

    let res = recv_response_by_id(&mut read, 1).await;
    let result = &res["result"];
    let capabilities = result["capabilities"]
        .as_array()
        .expect("capabilities array");
    assert!(
        capabilities
            .iter()
            .any(|c| c.as_str() == Some("categories")),
        "Expected 'categories' capability, got: {capabilities:?}"
    );

    // 2. Initially categories.list is empty
    send_json(
        &mut write,
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "categories.list",
        }),
    )
    .await;
    let res = recv_response_by_id(&mut read, 2).await;
    assert_eq!(res["result"].as_array().unwrap().len(), 0);

    // 3. Create two categories
    send_json(
        &mut write,
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "categories.create",
            "params": {
                "name": "Videos",
                "color": "#E91E63",
                "defaultDestination": "D:/Downloads/Videos",
                "defaultPriority": "high"
            }
        }),
    )
    .await;
    let res = recv_response_by_id(&mut read, 3).await;
    let cat_video = &res["result"];
    let video_id = cat_video["id"].as_str().unwrap().to_string();
    assert_eq!(cat_video["name"], "Videos");
    assert_eq!(cat_video["color"], "#E91E63");
    assert_eq!(cat_video["defaultDestination"], "D:/Downloads/Videos");
    assert_eq!(cat_video["defaultPriority"], "high");

    send_json(
        &mut write,
        json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "categories.create",
            "params": {
                "name": "Documents",
                "color": "#2196F3"
            }
        }),
    )
    .await;
    let res = recv_response_by_id(&mut read, 4).await;
    let doc_id = res["result"]["id"].as_str().unwrap().to_string();

    // 4. Update category
    send_json(
        &mut write,
        json!({
            "jsonrpc": "2.0",
            "id": 5,
            "method": "categories.update",
            "params": {
                "id": doc_id,
                "name": "Docs & PDF",
                "color": "#00BCD4",
                "defaultPriority": "low"
            }
        }),
    )
    .await;
    let res = recv_response_by_id(&mut read, 5).await;
    assert_eq!(res["result"]["name"], "Docs & PDF");
    assert_eq!(res["result"]["color"], "#00BCD4");
    assert_eq!(res["result"]["defaultPriority"], "low");

    // 5. Reorder categories
    send_json(
        &mut write,
        json!({
            "jsonrpc": "2.0",
            "id": 6,
            "method": "categories.reorder",
            "params": {
                "categoryIds": [&doc_id, &video_id]
            }
        }),
    )
    .await;
    let res = recv_response_by_id(&mut read, 6).await;
    assert_eq!(res["result"]["success"], true);

    send_json(
        &mut write,
        json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "categories.list",
        }),
    )
    .await;
    let res = recv_response_by_id(&mut read, 7).await;
    let list = res["result"].as_array().unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0]["id"], doc_id);
    assert_eq!(list[0]["sortOrder"], 0);
    assert_eq!(list[1]["id"], video_id);
    assert_eq!(list[1]["sortOrder"], 1);

    // 6. Add eD2K link with Category & 4-tier destination resolution
    let test_link = "ed2k://|file|sample_video.mkv|10485760|0123456789abcdef0123456789abcdef|/";
    send_json(
        &mut write,
        json!({
            "jsonrpc": "2.0",
            "id": 8,
            "method": "downloads.addEd2kLink",
            "params": {
                "link": test_link,
                "categoryId": video_id,
            }
        }),
    )
    .await;
    let res = recv_response_by_id(&mut read, 8).await;
    let download_id = res["result"]["downloadId"].as_str().unwrap().to_string();

    // Verify download details include categoryId and category default destination
    send_json(
        &mut write,
        json!({
            "jsonrpc": "2.0",
            "id": 9,
            "method": "downloads.get",
            "params": {
                "id": download_id
            }
        }),
    )
    .await;
    let res = recv_response_by_id(&mut read, 9).await;
    let dl_detail = &res["result"];
    assert_eq!(dl_detail["categoryId"], video_id);
    let target_path = dl_detail["targetPath"].as_str().unwrap();
    assert!(
        target_path.contains("sample_video.mkv"),
        "Expected targetPath to end with filename: {target_path}"
    );
    assert!(
        target_path.contains("Videos"),
        "Expected category destination 'Videos' to be resolved in tier 2: {target_path}"
    );

    // 7. Change category on active download via downloads.setCategory
    send_json(
        &mut write,
        json!({
            "jsonrpc": "2.0",
            "id": 10,
            "method": "downloads.setCategory",
            "params": {
                "id": download_id,
                "categoryId": doc_id
            }
        }),
    )
    .await;
    let res = recv_response_by_id(&mut read, 10).await;
    assert_eq!(res["result"]["categoryId"], doc_id);

    // 8. Delete category: MUST reassign assigned download to NULL, without deleting the download
    send_json(
        &mut write,
        json!({
            "jsonrpc": "2.0",
            "id": 11,
            "method": "categories.delete",
            "params": {
                "id": doc_id
            }
        }),
    )
    .await;
    let res = recv_response_by_id(&mut read, 11).await;
    assert_eq!(res["result"]["deleted"], true);
    assert_eq!(res["result"]["reassignedDownloads"], 1);

    // Check that the download is STILL ALIVE and its categoryId is null
    send_json(
        &mut write,
        json!({
            "jsonrpc": "2.0",
            "id": 12,
            "method": "downloads.get",
            "params": {
                "id": download_id
            }
        }),
    )
    .await;
    let res = recv_response_by_id(&mut read, 12).await;
    assert_eq!(res["result"]["id"], download_id);
    assert!(
        res["result"]["categoryId"].is_null(),
        "Expected categoryId to be null after category deletion, got: {:?}",
        res["result"]["categoryId"]
    );
}
