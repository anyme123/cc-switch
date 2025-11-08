use serde_json::{json, Value};
use std::collections::HashMap;

use crate::app_config::{AppType, McpConfig, MultiAppConfig};

/// 规范化 Windows 下的 npx 命令：自动添加 cmd /c 包裹
/// 修改传入的 spec，如果是 Windows 且 command 是 npx 但没有 cmd /c 包裹，则自动修复
fn normalize_windows_npx_command(spec: &mut Value) -> bool {
    // 仅在 Windows 平台处理
    #[cfg(not(target_os = "windows"))]
    {
        return false;
    }

    #[cfg(target_os = "windows")]
    {
        let Some(spec_obj) = spec.as_object_mut() else {
            return false;
        };

        // 仅处理 stdio 类型（默认也是 stdio）
        let typ = spec_obj
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("stdio");
        if typ != "stdio" {
            return false;
        }

        let Some(cmd) = spec_obj.get("command").and_then(|v| v.as_str()).map(|s| s.to_string()) else {
            return false;
        };

        let cmd_trimmed = cmd.trim();

        // 如果命令是 npx 开头，但不是 cmd /c 包裹的，则需要修复
        if cmd_trimmed.starts_with("npx ") || cmd_trimmed == "npx" {
            // 检查是否已经包裹
            if !cmd_trimmed.starts_with("cmd /c") && !cmd_trimmed.starts_with("cmd.exe /c") {
                // 先读取原来的 args（如果有）
                let old_args = spec_obj.get("args").and_then(|v| v.as_array()).cloned();

                // 需要修复：添加 cmd /c 包裹
                spec_obj.insert("command".into(), json!("cmd"));

                // 将原 command 和 args 合并为新的 args
                let mut new_args = vec![json!("/c"), json!(cmd_trimmed)];

                // 如果原来有 args，追加到后面
                if let Some(old_args_vec) = old_args {
                    new_args.extend(old_args_vec);
                }

                spec_obj.insert("args".into(), json!(new_args));

                log::info!("已自动修复 Windows npx 命令: {} -> cmd /c {}", cmd_trimmed, cmd_trimmed);
                return true;
            }
        }

        false
    }
}

/// 基础校验：允许 stdio/http；或省略 type（视为 stdio）。对应必填字段存在
fn validate_server_spec(spec: &Value) -> Result<(), String> {
    if !spec.is_object() {
        return Err("MCP 服务器连接定义必须为 JSON 对象".into());
    }
    let t_opt = spec.get("type").and_then(|x| x.as_str());
    // 支持两种：stdio/http；若缺省 type 则按 stdio 处理（与社区常见 .mcp.json 一致）
    let is_stdio = t_opt.map(|t| t == "stdio").unwrap_or(true);
    let is_http = t_opt.map(|t| t == "http").unwrap_or(false);

    if !(is_stdio || is_http) {
        return Err("MCP 服务器 type 必须是 'stdio' 或 'http'（或省略表示 stdio）".into());
    }

    if is_stdio {
        let cmd = spec.get("command").and_then(|x| x.as_str()).unwrap_or("");
        if cmd.trim().is_empty() {
            return Err("stdio 类型的 MCP 服务器缺少 command 字段".into());
        }
    }
    if is_http {
        let url = spec.get("url").and_then(|x| x.as_str()).unwrap_or("");
        if url.trim().is_empty() {
            return Err("http 类型的 MCP 服务器缺少 url 字段".into());
        }
    }
    Ok(())
}

fn validate_mcp_entry(entry: &Value) -> Result<(), String> {
    let obj = entry
        .as_object()
        .ok_or_else(|| "MCP 服务器条目必须为 JSON 对象".to_string())?;

    let server = obj
        .get("server")
        .ok_or_else(|| "MCP 服务器条目缺少 server 字段".to_string())?;
    validate_server_spec(server)?;

    for key in ["name", "description", "homepage", "docs"] {
        if let Some(val) = obj.get(key) {
            if !val.is_string() {
                return Err(format!("MCP 服务器 {} 必须为字符串", key));
            }
        }
    }

    if let Some(tags) = obj.get("tags") {
        let arr = tags
            .as_array()
            .ok_or_else(|| "MCP 服务器 tags 必须为字符串数组".to_string())?;
        if !arr.iter().all(|item| item.is_string()) {
            return Err("MCP 服务器 tags 必须为字符串数组".into());
        }
    }

    if let Some(enabled) = obj.get("enabled") {
        if !enabled.is_boolean() {
            return Err("MCP 服务器 enabled 必须为布尔值".into());
        }
    }

    Ok(())
}

fn normalize_server_keys(map: &mut HashMap<String, Value>) -> usize {
    let mut change_count = 0usize;
    let mut renames: Vec<(String, String)> = Vec::new();

    for (key_ref, value) in map.iter_mut() {
        let key = key_ref.clone();
        let Some(obj) = value.as_object_mut() else {
            continue;
        };

        let id_value = obj.get("id").cloned();

        let target_id: String;

        match id_value {
            Some(id_val) => match id_val.as_str() {
                Some(id_str) => {
                    let trimmed = id_str.trim();
                    if trimmed.is_empty() {
                        obj.insert("id".into(), json!(key.clone()));
                        change_count += 1;
                        target_id = key.clone();
                    } else {
                        if trimmed != id_str {
                            obj.insert("id".into(), json!(trimmed));
                            change_count += 1;
                        }
                        target_id = trimmed.to_string();
                    }
                }
                None => {
                    obj.insert("id".into(), json!(key.clone()));
                    change_count += 1;
                    target_id = key.clone();
                }
            },
            None => {
                obj.insert("id".into(), json!(key.clone()));
                change_count += 1;
                target_id = key.clone();
            }
        }

        if target_id != key {
            renames.push((key, target_id));
        }
    }

    for (old_key, new_key) in renames {
        if old_key == new_key {
            continue;
        }
        if map.contains_key(&new_key) {
            log::warn!(
                "MCP 条目 '{}' 的内部 id '{}' 与现有键冲突，回退为原键",
                old_key,
                new_key
            );
            if let Some(value) = map.get_mut(&old_key) {
                if let Some(obj) = value.as_object_mut() {
                    if obj
                        .get("id")
                        .and_then(|v| v.as_str())
                        .map(|s| s != old_key)
                        .unwrap_or(true)
                    {
                        obj.insert("id".into(), json!(old_key.clone()));
                        change_count += 1;
                    }
                }
            }
            continue;
        }
        if let Some(mut value) = map.remove(&old_key) {
            if let Some(obj) = value.as_object_mut() {
                obj.insert("id".into(), json!(new_key.clone()));
            }
            log::info!("MCP 条目键名已自动修复: '{}' -> '{}'", old_key, new_key);
            map.insert(new_key, value);
            change_count += 1;
        }
    }

    change_count
}

pub fn normalize_servers_for(config: &mut MultiAppConfig, app: &AppType) -> usize {
    let servers = &mut config.mcp_for_mut(app).servers;
    normalize_server_keys(servers)
}

fn extract_server_spec(entry: &Value) -> Result<Value, String> {
    let obj = entry
        .as_object()
        .ok_or_else(|| "MCP 服务器条目必须为 JSON 对象".to_string())?;
    let server = obj
        .get("server")
        .ok_or_else(|| "MCP 服务器条目缺少 server 字段".to_string())?;

    if !server.is_object() {
        return Err("MCP 服务器 server 字段必须为 JSON 对象".into());
    }

    Ok(server.clone())
}

/// 返回已启用的 MCP 服务器（过滤 enabled==true）
fn collect_enabled_servers(cfg: &McpConfig) -> HashMap<String, Value> {
    let mut out = HashMap::new();
    for (id, entry) in cfg.servers.iter() {
        let enabled = entry
            .get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if !enabled {
            continue;
        }
        match extract_server_spec(entry) {
            Ok(spec) => {
                out.insert(id.clone(), spec);
            }
            Err(err) => {
                log::warn!("跳过无效的 MCP 条目 '{}': {}", id, err);
            }
        }
    }
    out
}

pub fn get_servers_snapshot_for(
    config: &mut MultiAppConfig,
    app: &AppType,
) -> (HashMap<String, Value>, usize) {
    let normalized = normalize_servers_for(config, app);
    let mut snapshot = config.mcp_for(app).servers.clone();
    snapshot.retain(|id, value| {
        let Some(obj) = value.as_object_mut() else {
            log::warn!("跳过无效的 MCP 条目 '{}': 必须为 JSON 对象", id);
            return false;
        };

        obj.entry(String::from("id")).or_insert(json!(id));

        match validate_mcp_entry(value) {
            Ok(()) => true,
            Err(err) => {
                log::error!("config.json 中存在无效的 MCP 条目 '{}': {}", id, err);
                false
            }
        }
    });
    (snapshot, normalized)
}

pub fn upsert_in_config_for(
    config: &mut MultiAppConfig,
    app: &AppType,
    id: &str,
    spec: Value,
) -> Result<bool, String> {
    if id.trim().is_empty() {
        return Err("MCP 服务器 ID 不能为空".into());
    }
    normalize_servers_for(config, app);
    validate_mcp_entry(&spec)?;

    let mut entry_obj = spec
        .as_object()
        .cloned()
        .ok_or_else(|| "MCP 服务器条目必须为 JSON 对象".to_string())?;
    if let Some(existing_id) = entry_obj.get("id") {
        let Some(existing_id_str) = existing_id.as_str() else {
            return Err("MCP 服务器 id 必须为字符串".into());
        };
        if existing_id_str != id {
            return Err(format!(
                "MCP 服务器条目中的 id '{}' 与参数 id '{}' 不一致",
                existing_id_str, id
            ));
        }
    } else {
        entry_obj.insert(String::from("id"), json!(id));
    }

    let value = Value::Object(entry_obj);

    let servers = &mut config.mcp_for_mut(app).servers;
    let before = servers.get(id).cloned();
    servers.insert(id.to_string(), value);

    Ok(before.is_none())
}

pub fn delete_in_config_for(
    config: &mut MultiAppConfig,
    app: &AppType,
    id: &str,
) -> Result<bool, String> {
    if id.trim().is_empty() {
        return Err("MCP 服务器 ID 不能为空".into());
    }
    normalize_servers_for(config, app);
    let existed = config.mcp_for_mut(app).servers.remove(id).is_some();
    Ok(existed)
}

/// 设置启用状态并同步到 ~/.claude.json
pub fn set_enabled_and_sync_for(
    config: &mut MultiAppConfig,
    app: &AppType,
    id: &str,
    enabled: bool,
) -> Result<bool, String> {
    if id.trim().is_empty() {
        return Err("MCP 服务器 ID 不能为空".into());
    }
    normalize_servers_for(config, app);
    if let Some(spec) = config.mcp_for_mut(app).servers.get_mut(id) {
        // 写入 enabled 字段
        let mut obj = spec
            .as_object()
            .cloned()
            .ok_or_else(|| "MCP 服务器定义必须为 JSON 对象".to_string())?;
        obj.insert("enabled".into(), json!(enabled));
        *spec = Value::Object(obj);
    } else {
        // 若不存在则直接返回 false
        return Ok(false);
    }

    // 同步启用项
    match app {
        AppType::Claude => {
            // 将启用项投影到 ~/.claude.json
            sync_enabled_to_claude(config)?;
        }
        AppType::Codex => {
            // 将启用项投影到 ~/.codex/config.toml
            sync_enabled_to_codex(config)?;
        }
        AppType::Droid => {
            // 将启用项投影到 ~/.factory/mcp.json
            sync_enabled_to_droid(config)?;
        }
    }
    Ok(true)
}

/// 将 config.json 中 enabled==true 的项投影写入 ~/.claude.json
pub fn sync_enabled_to_claude(config: &MultiAppConfig) -> Result<(), String> {
    let enabled = collect_enabled_servers(&config.mcp.claude);
    crate::claude_mcp::set_mcp_servers_map(&enabled)
}

/// 从 ~/.claude.json 导入 mcpServers 到 config.json（设为 enabled=true）。
/// 已存在的项会更新 server 字段（同步系统配置的最新参数）和 enabled=true。
pub fn import_from_claude(config: &mut MultiAppConfig) -> Result<usize, String> {
    let text_opt = crate::claude_mcp::read_mcp_json()?;
    let Some(text) = text_opt else { return Ok(0) };
    let mut changed = normalize_servers_for(config, &AppType::Claude);
    let v: Value =
        serde_json::from_str(&text).map_err(|e| format!("解析 ~/.claude.json 失败: {}", e))?;
    let Some(map) = v.get("mcpServers").and_then(|x| x.as_object()) else {
        return Ok(changed);
    };

    for (id, spec) in map.iter() {
        // 规范化 Windows npx 命令并校验目标 spec
        let mut spec_normalized = spec.clone();
        normalize_windows_npx_command(&mut spec_normalized);
        validate_server_spec(&spec_normalized)?;

        let entry = config
            .mcp_for_mut(&AppType::Claude)
            .servers
            .entry(id.clone());
        use std::collections::hash_map::Entry;
        match entry {
            Entry::Vacant(vac) => {
                let mut obj = serde_json::Map::new();
                obj.insert(String::from("id"), json!(id));
                obj.insert(String::from("name"), json!(id));
                obj.insert(String::from("server"), spec_normalized.clone());
                obj.insert(String::from("enabled"), json!(true));
                vac.insert(Value::Object(obj));
                changed += 1;
            }
            Entry::Occupied(mut occ) => {
                let value = occ.get_mut();
                let Some(existing) = value.as_object_mut() else {
                    log::warn!("MCP 条目 '{}' 不是 JSON 对象，覆盖为导入数据", id);
                    let mut obj = serde_json::Map::new();
                    obj.insert(String::from("id"), json!(id));
                    obj.insert(String::from("name"), json!(id));
                    obj.insert(String::from("server"), spec_normalized.clone());
                    obj.insert(String::from("enabled"), json!(true));
                    occ.insert(Value::Object(obj));
                    changed += 1;
                    continue;
                };

                let mut modified = false;

                // 总是更新 server 字段，同步系统配置的最新参数
                let prev_server = existing.get("server");
                if prev_server.is_none() || prev_server != Some(&spec_normalized) {
                    existing.insert(String::from("server"), spec_normalized.clone());
                    modified = true;
                    log::debug!("更新 MCP '{}' 的 server 配置", id);
                }

                let prev_enabled = existing
                    .get("enabled")
                    .and_then(|b| b.as_bool())
                    .unwrap_or(false);
                if !prev_enabled {
                    existing.insert(String::from("enabled"), json!(true));
                    modified = true;
                }

                if existing.get("id").is_none() {
                    log::warn!("MCP 条目 '{}' 缺少 id 字段，自动填充", id);
                    existing.insert(String::from("id"), json!(id));
                    modified = true;
                }
                if modified {
                    changed += 1;
                }
            }
        }
    }
    Ok(changed)
}

/// 从 ~/.codex/config.toml 导入 MCP 到 config.json（Codex 作用域），并将导入项设为 enabled=true。
/// 支持两种 schema：[mcp.servers.<id>] 与 [mcp_servers.<id>]。
/// 已存在的项会更新 server 字段（同步系统配置的最新参数）和 enabled=true。
pub fn import_from_codex(config: &mut MultiAppConfig) -> Result<usize, String> {
    let text = crate::codex_config::read_and_validate_codex_config_text()?;
    if text.trim().is_empty() {
        return Ok(0);
    }
    let mut changed_total = normalize_servers_for(config, &AppType::Codex);

    let root: toml::Table =
        toml::from_str(&text).map_err(|e| format!("解析 ~/.codex/config.toml 失败: {}", e))?;

    // helper：处理一组 servers 表
    let mut import_servers_tbl = |servers_tbl: &toml::value::Table| {
        let mut changed = 0usize;
        for (id, entry_val) in servers_tbl.iter() {
            let Some(entry_tbl) = entry_val.as_table() else {
                continue;
            };

            // type 缺省为 stdio
            let typ = entry_tbl
                .get("type")
                .and_then(|v| v.as_str())
                .unwrap_or("stdio");

            // 构建 JSON 规范
            let mut spec = serde_json::Map::new();
            spec.insert("type".into(), json!(typ));

            match typ {
                "stdio" => {
                    if let Some(cmd) = entry_tbl.get("command").and_then(|v| v.as_str()) {
                        spec.insert("command".into(), json!(cmd));
                    }
                    if let Some(args) = entry_tbl.get("args").and_then(|v| v.as_array()) {
                        let arr = args
                            .iter()
                            .filter_map(|x| x.as_str())
                            .map(|s| json!(s))
                            .collect::<Vec<_>>();
                        if !arr.is_empty() {
                            spec.insert("args".into(), serde_json::Value::Array(arr));
                        }
                    }
                    if let Some(cwd) = entry_tbl.get("cwd").and_then(|v| v.as_str()) {
                        if !cwd.trim().is_empty() {
                            spec.insert("cwd".into(), json!(cwd));
                        }
                    }
                    if let Some(env_tbl) = entry_tbl.get("env").and_then(|v| v.as_table()) {
                        let mut env_json = serde_json::Map::new();
                        for (k, v) in env_tbl.iter() {
                            if let Some(sv) = v.as_str() {
                                env_json.insert(k.clone(), json!(sv));
                            }
                        }
                        if !env_json.is_empty() {
                            spec.insert("env".into(), serde_json::Value::Object(env_json));
                        }
                    }
                }
                "http" => {
                    if let Some(url) = entry_tbl.get("url").and_then(|v| v.as_str()) {
                        spec.insert("url".into(), json!(url));
                    }
                    if let Some(headers_tbl) = entry_tbl.get("headers").and_then(|v| v.as_table()) {
                        let mut headers_json = serde_json::Map::new();
                        for (k, v) in headers_tbl.iter() {
                            if let Some(sv) = v.as_str() {
                                headers_json.insert(k.clone(), json!(sv));
                            }
                        }
                        if !headers_json.is_empty() {
                            spec.insert("headers".into(), serde_json::Value::Object(headers_json));
                        }
                    }
                }
                _ => {}
            }

            let mut spec_v = serde_json::Value::Object(spec);

            // 规范化 Windows npx 命令并校验
            normalize_windows_npx_command(&mut spec_v);
            if let Err(e) = validate_server_spec(&spec_v) {
                log::warn!("跳过无效 Codex MCP 项 '{}': {}", id, e);
                continue;
            }

            // 合并：强制 enabled=true，并更新 server 字段
            use std::collections::hash_map::Entry;
            let entry = config
                .mcp_for_mut(&AppType::Codex)
                .servers
                .entry(id.clone());
            match entry {
                Entry::Vacant(vac) => {
                    let mut obj = serde_json::Map::new();
                    obj.insert(String::from("id"), json!(id));
                    obj.insert(String::from("name"), json!(id));
                    obj.insert(String::from("server"), spec_v.clone());
                    obj.insert(String::from("enabled"), json!(true));
                    vac.insert(serde_json::Value::Object(obj));
                    changed += 1;
                }
                Entry::Occupied(mut occ) => {
                    let value = occ.get_mut();
                    let Some(existing) = value.as_object_mut() else {
                        log::warn!("MCP 条目 '{}' 不是 JSON 对象，覆盖为导入数据", id);
                        let mut obj = serde_json::Map::new();
                        obj.insert(String::from("id"), json!(id));
                        obj.insert(String::from("name"), json!(id));
                        obj.insert(String::from("server"), spec_v.clone());
                        obj.insert(String::from("enabled"), json!(true));
                        occ.insert(serde_json::Value::Object(obj));
                        changed += 1;
                        continue;
                    };

                    let mut modified = false;

                    // 总是更新 server 字段，同步系统配置的最新参数
                    let prev_server = existing.get("server");
                    if prev_server.is_none() || prev_server != Some(&spec_v) {
                        existing.insert(String::from("server"), spec_v.clone());
                        modified = true;
                        log::debug!("更新 MCP '{}' 的 server 配置", id);
                    }

                    let prev = existing
                        .get("enabled")
                        .and_then(|b| b.as_bool())
                        .unwrap_or(false);
                    if !prev {
                        existing.insert(String::from("enabled"), json!(true));
                        modified = true;
                    }

                    if existing.get("id").is_none() {
                        log::warn!("MCP 条目 '{}' 缺少 id 字段，自动填充", id);
                        existing.insert(String::from("id"), json!(id));
                        modified = true;
                    }
                    if modified {
                        changed += 1;
                    }
                }
            }
        }
        changed
    };

    // 1) 处理 mcp.servers
    if let Some(mcp_val) = root.get("mcp") {
        if let Some(mcp_tbl) = mcp_val.as_table() {
            if let Some(servers_val) = mcp_tbl.get("servers") {
                if let Some(servers_tbl) = servers_val.as_table() {
                    changed_total += import_servers_tbl(servers_tbl);
                }
            }
        }
    }

    // 2) 处理 mcp_servers
    if let Some(servers_val) = root.get("mcp_servers") {
        if let Some(servers_tbl) = servers_val.as_table() {
            changed_total += import_servers_tbl(servers_tbl);
        }
    }

    Ok(changed_total)
}

/// 将 config.json 中 Codex 的 enabled==true 项以 TOML 形式合并到 ~/.codex/config.toml 的 [mcp.servers]
/// 策略：
/// - 读取现有 config.toml；若语法无效则报错，不尝试覆盖
/// - 保留系统中已存在但不在项目管理列表中的服务器（用户手动添加的）
/// - 更新/添加项目中 enabled=true 的服务器
/// - 保留 `mcp` 其它键
pub fn sync_enabled_to_codex(config: &MultiAppConfig) -> Result<(), String> {
    use toml::{value::Value as TomlValue, Table as TomlTable};

    // 1) 收集启用项（Codex 维度）
    let enabled = collect_enabled_servers(&config.mcp.codex);

    // 2) 读取现有 config.toml 并解析为 Table（允许空文件）
    let base_text = crate::codex_config::read_and_validate_codex_config_text()?;
    let mut root: TomlTable = if base_text.trim().is_empty() {
        TomlTable::new()
    } else {
        toml::from_str::<TomlTable>(&base_text)
            .map_err(|e| format!("解析 config.toml 失败: {}", e))?
    };

    // 3) 写入 servers 表（支持 mcp.servers 与 mcp_servers；优先沿用已有风格，默认 mcp_servers）
    let prefer_mcp_servers = root.get("mcp_servers").is_some() || root.get("mcp").is_none();

    // 读取现有的 servers 配置
    let existing_servers = if prefer_mcp_servers {
        root.get("mcp_servers")
            .and_then(|v| v.as_table())
            .cloned()
            .unwrap_or_default()
    } else {
        root.get("mcp")
            .and_then(|v| v.as_table())
            .and_then(|t| t.get("servers"))
            .and_then(|v| v.as_table())
            .cloned()
            .unwrap_or_default()
    };

    // 从现有配置开始，保留所有已存在的服务器
    let mut servers_tbl = existing_servers;

    // 仅添加系统中不存在的服务器（避免覆盖用户手动修改的参数）
    for (id, spec) in enabled.iter() {
        // 如果系统中已经存在这个服务器，保留系统中的配置
        if servers_tbl.contains_key(id) {
            log::debug!("保留系统中已存在的 MCP 服务器配置: {}", id);
            continue;
        }

        let mut s = TomlTable::new();

        // 类型（缺省视为 stdio）
        let typ = spec.get("type").and_then(|v| v.as_str()).unwrap_or("stdio");
        s.insert("type".into(), TomlValue::String(typ.to_string()));

        match typ {
            "stdio" => {
                let cmd = spec
                    .get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                s.insert("command".into(), TomlValue::String(cmd));

                if let Some(args) = spec.get("args").and_then(|v| v.as_array()) {
                    let arr = args
                        .iter()
                        .filter_map(|x| x.as_str())
                        .map(|x| TomlValue::String(x.to_string()))
                        .collect::<Vec<_>>();
                    if !arr.is_empty() {
                        s.insert("args".into(), TomlValue::Array(arr));
                    }
                }

                if let Some(cwd) = spec.get("cwd").and_then(|v| v.as_str()) {
                    if !cwd.trim().is_empty() {
                        s.insert("cwd".into(), TomlValue::String(cwd.to_string()));
                    }
                }

                if let Some(env) = spec.get("env").and_then(|v| v.as_object()) {
                    let mut env_tbl = TomlTable::new();
                    for (k, v) in env.iter() {
                        if let Some(sv) = v.as_str() {
                            env_tbl.insert(k.clone(), TomlValue::String(sv.to_string()));
                        }
                    }
                    if !env_tbl.is_empty() {
                        s.insert("env".into(), TomlValue::Table(env_tbl));
                    }
                }
            }
            "http" => {
                let url = spec
                    .get("url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                s.insert("url".into(), TomlValue::String(url));

                if let Some(headers) = spec.get("headers").and_then(|v| v.as_object()) {
                    let mut h_tbl = TomlTable::new();
                    for (k, v) in headers.iter() {
                        if let Some(sv) = v.as_str() {
                            h_tbl.insert(k.clone(), TomlValue::String(sv.to_string()));
                        }
                    }
                    if !h_tbl.is_empty() {
                        s.insert("headers".into(), TomlValue::Table(h_tbl));
                    }
                }
            }
            _ => {}
        }

        // 仅当系统中不存在时才插入
        servers_tbl.insert(id.clone(), TomlValue::Table(s));
        log::info!("添加新的 MCP 服务器到系统配置: {}", id);
    }

    // 4) 写入合并后的 servers 表
    let servers_value = TomlValue::Table(servers_tbl.clone());

    if prefer_mcp_servers {
        root.insert("mcp_servers".into(), servers_value);

        // 若存在 mcp，则仅移除 servers 字段，保留其他键
        let mut should_drop_mcp = false;
        if let Some(mcp_val) = root.get_mut("mcp") {
            match mcp_val {
                TomlValue::Table(tbl) => {
                    tbl.remove("servers");
                    should_drop_mcp = tbl.is_empty();
                }
                _ => should_drop_mcp = true,
            }
        }
        if should_drop_mcp {
            root.remove("mcp");
        }
    } else {
        let mut inserted = false;

        if let Some(mcp_val) = root.get_mut("mcp") {
            match mcp_val {
                TomlValue::Table(tbl) => {
                    tbl.insert("servers".into(), TomlValue::Table(servers_tbl.clone()));
                    inserted = true;
                }
                _ => {
                    let mut mcp_tbl = TomlTable::new();
                    mcp_tbl.insert("servers".into(), TomlValue::Table(servers_tbl.clone()));
                    *mcp_val = TomlValue::Table(mcp_tbl);
                    inserted = true;
                }
            }
        }

        if !inserted {
            let mut mcp_tbl = TomlTable::new();
            mcp_tbl.insert("servers".into(), TomlValue::Table(servers_tbl));
            root.insert("mcp".into(), TomlValue::Table(mcp_tbl));
        }

        root.remove("mcp_servers");
    }

    // 5) 序列化并写回 config.toml（仅改 TOML，不触碰 auth.json）
    let new_text = toml::to_string(&TomlValue::Table(root))
        .map_err(|e| format!("序列化 config.toml 失败: {}", e))?;
    let path = crate::codex_config::get_codex_config_path();
    crate::config::write_text_file(&path, &new_text)?;

    Ok(())
}

// =====================
// v3.5.1 新增：MCP 配置双端同步功能
// =====================

/// 检查目标应用中是否存在同名 MCP 服务器
pub fn check_mcp_exists_in_target_app(
    config: &MultiAppConfig,
    target_app: &AppType,
    id: &str,
) -> bool {
    config.mcp_for(target_app).servers.contains_key(id)
}

/// 将 MCP 服务器配置复制到目标应用（三向同步：Claude ↔ Codex ↔ Droid）
///
/// # 参数
/// - `config`: 应用配置
/// - `source_app`: 源应用类型 (Claude、Codex 或 Droid)
/// - `target_app`: 目标应用类型 (Claude、Codex 或 Droid)
/// - `id`: MCP 服务器 ID
/// - `overwrite`: 是否覆盖已存在的同名配置
///
/// # 返回
/// - `Ok(true)`: 成功复制（新增或覆盖）
/// - `Ok(false)`: 目标已存在且不覆盖
/// - `Err`: 复制失败
pub fn copy_mcp_to_target_app(
    config: &mut MultiAppConfig,
    source_app: &AppType,
    target_app: &AppType,
    id: &str,
    overwrite: bool,
) -> Result<bool, String> {
    // 1. 验证源和目标不能相同
    if source_app == target_app {
        return Err(format!("源应用和目标应用不能相同: {:?}", source_app));
    }

    // 2. 从源应用获取 MCP 配置
    let source_entry = config
        .mcp_for(source_app)
        .servers
        .get(id)
        .ok_or_else(|| format!("源 MCP 服务器 '{}' 不存在", id))?
        .clone();

    // 3. 检查目标应用是否已存在同名配置
    let exists = config.mcp_for(target_app).servers.contains_key(id);
    if exists && !overwrite {
        return Ok(false);
    }

    // 4. 复制配置到目标应用
    // 注意：复制时不启用，保持 enabled=false（或源配置的值）
    let mut target_entry = source_entry.clone();

    // 确保 enabled 字段存在且为 false（或保持源值）
    if let Some(obj) = target_entry.as_object_mut() {
        // 如果是新增，默认不启用；如果是覆盖，保持源的启用状态
        if !exists {
            obj.insert("enabled".into(), json!(false));
        }
    }

    config
        .mcp_for_mut(target_app)
        .servers
        .insert(id.to_string(), target_entry);

    log::info!(
        "已将 MCP '{}' 从 {:?} 同步到 {:?} (覆盖: {})",
        id,
        source_app,
        target_app,
        overwrite
    );

    Ok(true)
}

/// 检查 MCP 是否已启用
pub fn is_mcp_enabled(config: &MultiAppConfig, app: &AppType, id: &str) -> bool {
    config
        .mcp_for(app)
        .servers
        .get(id)
        .and_then(|entry| entry.get("enabled"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

// =====================
// Droid/Factory MCP 功能
// =====================

/// 将 config.json 中 enabled==true 的项投影写入 ~/.factory/mcp.json
pub fn sync_enabled_to_droid(config: &MultiAppConfig) -> Result<(), String> {
    let enabled = collect_enabled_servers(&config.mcp.droid);
    crate::factory_mcp::set_mcp_servers_map(&enabled)
}

/// 从 ~/.factory/mcp.json 导入 mcpServers 到 config.json（设为 enabled=true）。
/// 已存在的项会更新 server 字段（同步系统配置的最新参数）和 enabled=true。
pub fn import_from_droid(config: &mut MultiAppConfig) -> Result<usize, String> {
    let text_opt = crate::factory_mcp::read_mcp_json()?;
    let Some(text) = text_opt else { return Ok(0) };
    let mut changed = normalize_servers_for(config, &AppType::Droid);
    let v: Value =
        serde_json::from_str(&text).map_err(|e| format!("解析 ~/.factory/mcp.json 失败: {}", e))?;
    let Some(map) = v.get("mcpServers").and_then(|x| x.as_object()) else {
        return Ok(changed);
    };

    for (id, spec) in map.iter() {
        // 规范化 Windows npx 命令并校验目标 spec
        let mut spec_normalized = spec.clone();
        normalize_windows_npx_command(&mut spec_normalized);
        validate_server_spec(&spec_normalized)?;

        let entry = config
            .mcp_for_mut(&AppType::Droid)
            .servers
            .entry(id.clone());
        use std::collections::hash_map::Entry;
        match entry {
            Entry::Vacant(vac) => {
                let mut obj = serde_json::Map::new();
                obj.insert(String::from("id"), json!(id));
                obj.insert(String::from("name"), json!(id));
                obj.insert(String::from("server"), spec_normalized.clone());
                obj.insert(String::from("enabled"), json!(true));
                vac.insert(Value::Object(obj));
                changed += 1;
            }
            Entry::Occupied(mut occ) => {
                let value = occ.get_mut();
                let Some(existing) = value.as_object_mut() else {
                    log::warn!("MCP 条目 '{}' 不是 JSON 对象，覆盖为导入数据", id);
                    let mut obj = serde_json::Map::new();
                    obj.insert(String::from("id"), json!(id));
                    obj.insert(String::from("name"), json!(id));
                    obj.insert(String::from("server"), spec_normalized.clone());
                    obj.insert(String::from("enabled"), json!(true));
                    occ.insert(Value::Object(obj));
                    changed += 1;
                    continue;
                };

                let mut modified = false;

                // 总是更新 server 字段，同步系统配置的最新参数
                let prev_server = existing.get("server");
                if prev_server.is_none() || prev_server != Some(&spec_normalized) {
                    existing.insert(String::from("server"), spec_normalized.clone());
                    modified = true;
                    log::debug!("更新 MCP '{}' 的 server 配置", id);
                }

                let prev_enabled = existing
                    .get("enabled")
                    .and_then(|b| b.as_bool())
                    .unwrap_or(false);
                if !prev_enabled {
                    existing.insert(String::from("enabled"), json!(true));
                    modified = true;
                }

                if existing.get("id").is_none() {
                    log::warn!("MCP 条目 '{}' 缺少 id 字段，自动填充", id);
                    existing.insert(String::from("id"), json!(id));
                    modified = true;
                }
                if modified {
                    changed += 1;
                }
            }
        }
    }
    Ok(changed)
}
