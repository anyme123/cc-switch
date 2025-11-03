use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 余额信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BalanceInfo {
    pub used: u64,
    pub allowance: u64,
    pub remaining: u64,
    pub overage: u64,
    pub used_ratio: f64,
    pub percent_used: f64,
    pub exceeded: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry_date: Option<String>,
}

/// Factory.ai API 响应结构
#[derive(Debug, Deserialize)]
struct FactoryApiResponse {
    usage: Usage,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Usage {
    standard: StandardUsage,
    #[serde(rename = "endDate")]
    end_date: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StandardUsage {
    user_tokens: u64,
    total_allowance: u64,
    org_overage_used: u64,
    used_ratio: f64,
}

/// 获取 Droid 配置目录路径
/// Factory AI 使用 ~/.factory 作为配置目录
pub fn get_droid_config_dir() -> PathBuf {
    if let Some(custom) = crate::settings::get_droid_override_dir() {
        return custom;
    }

    dirs::home_dir()
        .expect("无法获取用户主目录")
        .join(".factory")
}

/// 获取 Droid API Key 配置文件路径（预留功能）
#[allow(dead_code)]
pub fn get_droid_api_key_path() -> PathBuf {
    get_droid_config_dir().join("api_key.txt")
}

/// 设置系统环境变量 Factory_API_Key
pub fn set_factory_api_key_env(api_key: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        set_windows_env_var("Factory_API_Key", api_key)
    }

    #[cfg(not(target_os = "windows"))]
    {
        set_unix_env_var("Factory_API_Key", api_key)
    }
}

/// 清除系统环境变量 Factory_API_Key
pub fn clear_factory_api_key_env() -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        clear_windows_env_var("Factory_API_Key")
    }

    #[cfg(not(target_os = "windows"))]
    {
        clear_unix_env_var("Factory_API_Key")
    }
}

/// 获取当前系统环境变量 Factory_API_Key 的值
pub fn get_factory_api_key_env() -> Result<Option<String>, String> {
    #[cfg(target_os = "windows")]
    {
        get_windows_env_var("Factory_API_Key")
    }

    #[cfg(not(target_os = "windows"))]
    {
        // Unix: 从当前进程环境中读取（已设置的话）
        Ok(std::env::var("Factory_API_Key").ok())
    }
}

/// Windows: 设置用户环境变量
#[cfg(target_os = "windows")]
fn set_windows_env_var(name: &str, value: &str) -> Result<(), String> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let env = hkcu
        .open_subkey_with_flags("Environment", KEY_SET_VALUE)
        .map_err(|e| format!("无法打开注册表环境变量键: {}", e))?;

    env.set_value(name, &value)
        .map_err(|e| format!("设置环境变量失败: {}", e))?;

    // 广播环境变量更改消息
    broadcast_env_change();

    log::info!("已设置 Windows 用户环境变量: {} = {}", name, value);
    Ok(())
}

/// Windows: 读取用户环境变量
#[cfg(target_os = "windows")]
fn get_windows_env_var(name: &str) -> Result<Option<String>, String> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let env = hkcu
        .open_subkey_with_flags("Environment", KEY_READ)
        .map_err(|e| format!("无法打开注册表环境变量键: {}", e))?;

    match env.get_value::<String, _>(name) {
        Ok(value) => Ok(Some(value)),
        Err(_) => Ok(None),
    }
}

/// Windows: 清除用户环境变量
#[cfg(target_os = "windows")]
fn clear_windows_env_var(name: &str) -> Result<(), String> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let env = hkcu
        .open_subkey_with_flags("Environment", KEY_SET_VALUE)
        .map_err(|e| format!("无法打开注册表环境变量键: {}", e))?;

    // 如果环境变量不存在，也算成功
    match env.delete_value(name) {
        Ok(_) => {
            // 广播环境变量更改消息
            broadcast_env_change();
            log::info!("已清除 Windows 用户环境变量: {}", name);
            Ok(())
        }
        Err(e) => {
            // 如果是"未找到"错误，也算成功
            if e.kind() == std::io::ErrorKind::NotFound {
                log::info!("环境变量 {} 不存在，无需清除", name);
                Ok(())
            } else {
                Err(format!("删除环境变量失败: {}", e))
            }
        }
    }
}

/// Windows: 广播环境变量更改消息
#[cfg(target_os = "windows")]
fn broadcast_env_change() {
    // 注意：这需要额外的 Windows API 调用，暂时省略
    // 用户可能需要重启终端或应用才能看到环境变量变化
    log::warn!("环境变量已更新，可能需要重启终端或应用以生效");
}

/// Unix-like: 设置环境变量（通过修改 shell 配置文件）
#[cfg(not(target_os = "windows"))]
fn set_unix_env_var(name: &str, value: &str) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("无法获取用户主目录")?;
    
    // 尝试写入多个常见的 shell 配置文件
    let shell_configs = vec![
        home.join(".bashrc"),
        home.join(".zshrc"),
        home.join(".profile"),
    ];

    let export_line = format!("export {}=\"{}\"", name, value);
    let marker_start = format!("# CC-Switch Droid Config Start");
    let marker_end = format!("# CC-Switch Droid Config End");

    for config_path in shell_configs {
        if config_path.exists() {
            match std::fs::read_to_string(&config_path) {
                Ok(content) => {
                    // 检查是否已存在配置
                    let new_content = if content.contains(&marker_start) {
                        // 替换现有配置
                        let re = regex::Regex::new(&format!(
                            r"(?s){}.*?{}",
                            regex::escape(&marker_start),
                            regex::escape(&marker_end)
                        ))
                        .unwrap();
                        re.replace(&content, format!("{}\n{}\n{}", marker_start, export_line, marker_end)).to_string()
                    } else {
                        // 添加新配置
                        format!("{}\n\n{}\n{}\n{}\n", content.trim_end(), marker_start, export_line, marker_end)
                    };

                    std::fs::write(&config_path, new_content)
                        .map_err(|e| format!("写入 {} 失败: {}", config_path.display(), e))?;

                    log::info!("已更新环境变量到: {}", config_path.display());
                }
                Err(e) => {
                    log::warn!("读取 {} 失败: {}", config_path.display(), e);
                }
            }
        }
    }

    log::info!("已设置环境变量: {} = {}（需要重启终端生效）", name, value);
    Ok(())
}

/// Unix-like: 清除环境变量
#[cfg(not(target_os = "windows"))]
fn clear_unix_env_var(_name: &str) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("无法获取用户主目录")?;
    
    let shell_configs = vec![
        home.join(".bashrc"),
        home.join(".zshrc"),
        home.join(".profile"),
    ];

    let marker_start = format!("# CC-Switch Droid Config Start");
    let marker_end = format!("# CC-Switch Droid Config End");

    for config_path in shell_configs {
        if config_path.exists() {
            match std::fs::read_to_string(&config_path) {
                Ok(content) => {
                    if content.contains(&marker_start) {
                        let re = regex::Regex::new(&format!(
                            r"(?s)\n*{}.*?{}\n*",
                            regex::escape(&marker_start),
                            regex::escape(&marker_end)
                        ))
                        .unwrap();
                        let new_content = re.replace(&content, "").to_string();

                        std::fs::write(&config_path, new_content)
                            .map_err(|e| format!("写入 {} 失败: {}", config_path.display(), e))?;

                        log::info!("已从 {} 移除环境变量配置", config_path.display());
                    }
                }
                Err(e) => {
                    log::warn!("读取 {} 失败: {}", config_path.display(), e);
                }
            }
        }
    }

    log::info!("已清除环境变量配置（需要重启终端生效）");
    Ok(())
}

/// 查询单个 API Key 的余额
pub async fn check_balance(api_key: &str) -> Result<BalanceInfo, String> {
    log::info!("开始查询余额，API Key: {}...", &api_key[..api_key.len().min(10)]);
    
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| {
            log::error!("创建 HTTP 客户端失败: {}", e);
            format!("创建 HTTP 客户端失败: {}", e)
        })?;

    log::info!("发送请求到 Factory.ai API...");
    let response = client
        .get("https://app.factory.ai/api/organization/members/chat-usage")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("x-factory-client", "web-browser")
        .header("Content-Type", "application/json")
        .send()
        .await
        .map_err(|e| {
            log::error!("请求失败: {}", e);
            format!("请求失败: {}", e)
        })?;

    let status = response.status();
    log::info!("收到响应，状态码: {}", status);
    
    if status.as_u16() != 200 {
        let text = response.text().await.unwrap_or_default();
        log::error!("HTTP 错误 {}: {}", status, text);
        return Err(format!("HTTP {}: {}", status, text));
    }

    // 先获取响应文本用于调试
    let response_text = response.text().await
        .map_err(|e| {
            log::error!("读取响应文本失败: {}", e);
            format!("读取响应失败: {}", e)
        })?;
    
    log::info!("响应内容: {}", response_text);

    // 解析 JSON
    let api_response: FactoryApiResponse = serde_json::from_str(&response_text)
        .map_err(|e| {
            log::error!("JSON 解析失败: {}，响应内容: {}", e, response_text);
            format!("解析响应失败: {}，原始响应: {}", e, response_text)
        })?;

    let usage = api_response.usage.standard;
    let remaining = usage.total_allowance.saturating_sub(usage.user_tokens);
    let percent_used = usage.used_ratio * 100.0;
    let exceeded = usage.used_ratio > 1.0;

    // 提取到期时间：endDate字段（Unix时间戳，毫秒）
    let expiry_date = api_response.usage.end_date.map(|timestamp| {
        // 将毫秒时间戳转换为ISO格式字符串
        use chrono::{Utc, TimeZone};
        let datetime = Utc.timestamp_millis_opt(timestamp).unwrap();
        datetime.to_rfc3339()
    });

    log::info!("余额查询成功: 已用 {}, 总配额 {}, 剩余 {}",
        usage.user_tokens, usage.total_allowance, remaining);

    if let Some(ref expiry) = expiry_date {
        log::info!("到期时间: {}", expiry);
    }

    Ok(BalanceInfo {
        used: usage.user_tokens,
        allowance: usage.total_allowance,
        remaining,
        overage: usage.org_overage_used,
        used_ratio: usage.used_ratio,
        percent_used,
        exceeded,
        expiry_date,
    })
}

/// 批量查询 API Keys 的余额
pub async fn batch_check_balances(
    api_keys: Vec<String>,
) -> Result<std::collections::HashMap<String, BalanceInfo>, String> {
    use std::collections::HashMap;

    let mut results = HashMap::new();

    for (i, key) in api_keys.iter().enumerate() {
        match check_balance(key).await {
            Ok(balance) => {
                results.insert(key.clone(), balance);
            }
            Err(e) => {
                log::warn!("查询密钥 {} 余额失败: {}", key, e);
            }
        }

        // 延迟 200ms 避免请求过快
        if i < api_keys.len() - 1 {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
    }

    Ok(results)
}

