use serde_json::{Map, Value};
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::atomic_write;
use crate::droid_config::get_droid_config_dir;

/// 获取 Factory/Droid MCP 配置文件路径：~/.factory/mcp.json
fn factory_mcp_config_path() -> PathBuf {
    get_droid_config_dir().join("mcp.json")
}

fn read_json_value(path: &Path) -> Result<Value, String> {
    if !path.exists() {
        return Ok(serde_json::json!({}));
    }
    let content =
        fs::read_to_string(path).map_err(|e| format!("读取文件失败: {}: {}", path.display(), e))?;
    let value: Value = serde_json::from_str(&content)
        .map_err(|e| format!("解析 JSON 失败: {}: {}", path.display(), e))?;
    Ok(value)
}

fn write_json_value(path: &Path, value: &Value) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("创建目录失败: {}: {}", parent.display(), e))?;
    }
    let json =
        serde_json::to_string_pretty(value).map_err(|e| format!("序列化 JSON 失败: {}", e))?;
    atomic_write(path, json.as_bytes())
}

/// 读取 Factory MCP 配置文件内容
pub fn read_mcp_json() -> Result<Option<String>, String> {
    let path = factory_mcp_config_path();
    log::info!("尝试读取 Droid MCP 配置: {}", path.display());

    if !path.exists() {
        log::warn!("Droid MCP 配置文件不存在: {}", path.display());
        return Ok(None);
    }

    log::info!("找到 Droid MCP 配置文件,开始读取...");
    let content = fs::read_to_string(&path)
        .map_err(|e| {
            log::error!("读取 Droid MCP 配置失败: {} - {}", path.display(), e);
            format!("读取 Factory MCP 配置失败: {}", e)
        })?;

    log::info!("成功读取 Droid MCP 配置,大小: {} 字节", content.len());
    Ok(Some(content))
}

/// 将给定的启用 MCP 服务器映射合并到 ~/.factory/mcp.json 的 mcpServers 字段
/// 策略：
/// - 系统配置优先：保留系统中已存在的服务器配置（用户可能手动修改过参数）
/// - 仅添加系统中不存在的项目服务器
/// - 其他字段保持不变
pub fn set_mcp_servers_map(
    servers: &std::collections::HashMap<String, Value>,
) -> Result<(), String> {
    let path = factory_mcp_config_path();
    let mut root = if path.exists() {
        read_json_value(&path)?
    } else {
        serde_json::json!({})
    };

    // 读取现有的 mcpServers（如果存在）
    let existing_servers = root
        .get("mcpServers")
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();

    // 从现有配置开始，保留所有已存在的服务器
    let mut out: Map<String, Value> = existing_servers;

    // 仅添加系统中不存在的服务器（避免覆盖用户手动修改的参数）
    for (id, spec) in servers.iter() {
        // 如果系统中已经存在这个服务器，保留系统中的配置
        if out.contains_key(id) {
            log::debug!("保留系统中已存在的 MCP 服务器配置: {}", id);
            continue;
        }

        let mut obj = if let Some(map) = spec.as_object() {
            map.clone()
        } else {
            return Err(format!("MCP 服务器 '{}' 不是对象", id));
        };

        // 如果存在 server 字段，提取其内容作为 spec
        if let Some(server_val) = obj.remove("server") {
            let server_obj = server_val
                .as_object()
                .cloned()
                .ok_or_else(|| format!("MCP 服务器 '{}' server 字段不是对象", id))?;
            obj = server_obj;
        }

        // 移除 UI 辅助字段
        obj.remove("enabled");
        obj.remove("source");
        obj.remove("id");
        obj.remove("name");
        obj.remove("description");
        obj.remove("tags");
        obj.remove("homepage");
        obj.remove("docs");

        // 仅当系统中不存在时才插入
        out.insert(id.clone(), Value::Object(obj));
        log::info!("添加新的 MCP 服务器到系统配置: {}", id);
    }

    {
        let obj = root
            .as_object_mut()
            .ok_or_else(|| "~/.factory/mcp.json 根必须是对象".to_string())?;
        obj.insert("mcpServers".into(), Value::Object(out));
    }

    write_json_value(&path, &root)?;
    Ok(())
}
