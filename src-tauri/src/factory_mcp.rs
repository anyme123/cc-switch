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
    if !path.exists() {
        return Ok(None);
    }
    let content = fs::read_to_string(&path)
        .map_err(|e| format!("读取 Factory MCP 配置失败: {}", e))?;
    Ok(Some(content))
}

/// 将给定的启用 MCP 服务器映射写入到 ~/.factory/mcp.json 的 mcpServers 字段
/// 仅覆盖 mcpServers，其他字段保持不变
pub fn set_mcp_servers_map(
    servers: &std::collections::HashMap<String, Value>,
) -> Result<(), String> {
    let path = factory_mcp_config_path();
    let mut root = if path.exists() {
        read_json_value(&path)?
    } else {
        serde_json::json!({})
    };

    // 构建 mcpServers 对象：移除 UI 辅助字段（enabled/source），仅保留实际 MCP 规范
    let mut out: Map<String, Value> = Map::new();
    for (id, spec) in servers.iter() {
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

        out.insert(id.clone(), Value::Object(obj));
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
