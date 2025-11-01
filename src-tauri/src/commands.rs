#![allow(non_snake_case)]

use std::collections::HashMap;
use std::str::FromStr;
use tauri::State;
use tauri_plugin_autostart::ManagerExt as AutostartManagerExt;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};
use tauri_plugin_opener::OpenerExt;

use crate::app_config::AppType;
use crate::claude_plugin;
use crate::codex_config;
use crate::config::{self, get_claude_settings_path, ConfigStatus};
use crate::provider::Provider;
use crate::store::AppState;
use crate::vscode;

fn validate_provider_settings(app_type: &AppType, provider: &Provider) -> Result<(), String> {
    match app_type {
        AppType::Claude => {
            if !provider.settings_config.is_object() {
                return Err("Claude 配置必须是 JSON 对象".to_string());
            }
        }
        AppType::Codex => {
            let settings = provider
                .settings_config
                .as_object()
                .ok_or_else(|| "Codex 配置必须是 JSON 对象".to_string())?;
            let auth = settings
                .get("auth")
                .ok_or_else(|| "Codex 配置缺少 auth 字段".to_string())?;
            if !auth.is_object() {
                return Err("Codex auth 配置必须是 JSON 对象".to_string());
            }
            if let Some(config_value) = settings.get("config") {
                if !(config_value.is_string() || config_value.is_null()) {
                    return Err("Codex config 字段必须是字符串".to_string());
                }
                if let Some(cfg_text) = config_value.as_str() {
                    codex_config::validate_config_toml(cfg_text)?;
                }
            }
        }
        AppType::Droid => {
            let settings = provider
                .settings_config
                .as_object()
                .ok_or_else(|| "Droid 配置必须是 JSON 对象".to_string())?;
            let api_key = settings
                .get("apiKey")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "Droid 配置缺少 apiKey 字段".to_string())?;
            if !api_key.starts_with("fk-") {
                return Err("Droid API Key 格式错误，应以 fk- 开头".to_string());
            }
        }
    }
    Ok(())
}

/// 获取所有供应商
#[tauri::command]
pub async fn get_providers(
    state: State<'_, AppState>,
    app_type: Option<AppType>,
    app: Option<String>,
    appType: Option<String>,
) -> Result<HashMap<String, Provider>, String> {
    let app_type = app_type
        .or_else(|| app.as_deref().map(|s| s.into()))
        .or_else(|| appType.as_deref().map(|s| s.into()))
        .unwrap_or(AppType::Claude);

    let config = state
        .config
        .lock()
        .map_err(|e| format!("获取锁失败: {}", e))?;

    let manager = config
        .get_manager(&app_type)
        .ok_or_else(|| format!("应用类型不存在: {:?}", app_type))?;

    Ok(manager.get_all_providers().clone())
}

/// 获取当前供应商ID
#[tauri::command]
pub async fn get_current_provider(
    state: State<'_, AppState>,
    app_type: Option<AppType>,
    app: Option<String>,
    appType: Option<String>,
) -> Result<String, String> {
    let app_type = app_type
        .or_else(|| app.as_deref().map(|s| s.into()))
        .or_else(|| appType.as_deref().map(|s| s.into()))
        .unwrap_or(AppType::Claude);

    let config = state
        .config
        .lock()
        .map_err(|e| format!("获取锁失败: {}", e))?;

    let manager = config
        .get_manager(&app_type)
        .ok_or_else(|| format!("应用类型不存在: {:?}", app_type))?;

    Ok(manager.current.clone())
}

/// 添加供应商
#[tauri::command]
pub async fn add_provider(
    state: State<'_, AppState>,
    app_type: Option<AppType>,
    app: Option<String>,
    appType: Option<String>,
    provider: Provider,
) -> Result<bool, String> {
    let app_type = app_type
        .or_else(|| app.as_deref().map(|s| s.into()))
        .or_else(|| appType.as_deref().map(|s| s.into()))
        .unwrap_or(AppType::Claude);

    validate_provider_settings(&app_type, &provider)?;

    // 读取当前是否是激活供应商（短锁）
    let is_current = {
        let config = state
            .config
            .lock()
            .map_err(|e| format!("获取锁失败: {}", e))?;
        let manager = config
            .get_manager(&app_type)
            .ok_or_else(|| format!("应用类型不存在: {:?}", app_type))?;
        manager.current == provider.id
    };

    // 若目标为当前供应商，则先写 live，成功后再落盘配置
    if is_current {
        match app_type {
            AppType::Claude => {
                use crate::config::{read_json_file, write_json_file};
                let settings_path = crate::config::get_claude_settings_path();

                // 读取现有配置（如果存在）
                let mut final_config = if settings_path.exists() {
                    read_json_file::<serde_json::Value>(&settings_path).unwrap_or(serde_json::json!({}))
                } else {
                    serde_json::json!({})
                };

                // 只更新 env 中的 ANTHROPIC_AUTH_TOKEN 和 ANTHROPIC_BASE_URL
                if let Some(provider_env) = provider.settings_config.get("env") {
                    if let Some(config_obj) = final_config.as_object_mut() {
                        // 获取或创建 env 对象
                        let env = config_obj.entry("env").or_insert(serde_json::json!({}));
                        if let Some(env_obj) = env.as_object_mut() {
                            // 只更新两个特定字段
                            if let Some(token) = provider_env.get("ANTHROPIC_AUTH_TOKEN") {
                                env_obj.insert("ANTHROPIC_AUTH_TOKEN".to_string(), token.clone());
                            }
                            if let Some(base_url) = provider_env.get("ANTHROPIC_BASE_URL") {
                                env_obj.insert("ANTHROPIC_BASE_URL".to_string(), base_url.clone());
                            }
                        }
                    }
                }

                // 写入合并后的配置
                write_json_file(&settings_path, &final_config)?;
            }
            AppType::Codex => {
                let auth = provider
                    .settings_config
                    .get("auth")
                    .ok_or_else(|| "目标供应商缺少 auth 配置".to_string())?;
                let cfg_text = provider
                    .settings_config
                    .get("config")
                    .and_then(|v| v.as_str());
                crate::codex_config::write_codex_live_atomic(auth, cfg_text)?;
            }
            AppType::Droid => {
                // Droid: 设置环境变量
                let api_key = provider
                    .settings_config
                    .get("apiKey")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Droid 配置缺少 apiKey 字段".to_string())?;
                crate::droid_config::set_factory_api_key_env(api_key)?;
            }
        }
    }

    // 更新内存并保存配置
    {
        let mut config = state
            .config
            .lock()
            .map_err(|e| format!("获取锁失败: {}", e))?;
        let manager = config
            .get_manager_mut(&app_type)
            .ok_or_else(|| format!("应用类型不存在: {:?}", app_type))?;
        manager
            .providers
            .insert(provider.id.clone(), provider.clone());
    }
    state.save()?;

    Ok(true)
}

/// 更新供应商
#[tauri::command]
pub async fn update_provider(
    state: State<'_, AppState>,
    app_type: Option<AppType>,
    app: Option<String>,
    appType: Option<String>,
    provider: Provider,
) -> Result<bool, String> {
    let app_type = app_type
        .or_else(|| app.as_deref().map(|s| s.into()))
        .or_else(|| appType.as_deref().map(|s| s.into()))
        .unwrap_or(AppType::Claude);

    validate_provider_settings(&app_type, &provider)?;

    // 读取校验 & 是否当前（短锁）
    let (exists, is_current) = {
        let config = state
            .config
            .lock()
            .map_err(|e| format!("获取锁失败: {}", e))?;
        let manager = config
            .get_manager(&app_type)
            .ok_or_else(|| format!("应用类型不存在: {:?}", app_type))?;
        (
            manager.providers.contains_key(&provider.id),
            manager.current == provider.id,
        )
    };
    if !exists {
        return Err(format!("供应商不存在: {}", provider.id));
    }

    // 若更新的是当前供应商，先写 live 成功再保存
    if is_current {
        match app_type {
            AppType::Claude => {
                use crate::config::{read_json_file, write_json_file};
                let settings_path = crate::config::get_claude_settings_path();

                // 读取现有配置（如果存在）
                let mut final_config = if settings_path.exists() {
                    read_json_file::<serde_json::Value>(&settings_path).unwrap_or(serde_json::json!({}))
                } else {
                    serde_json::json!({})
                };

                // 只更新 env 中的 ANTHROPIC_AUTH_TOKEN 和 ANTHROPIC_BASE_URL
                if let Some(provider_env) = provider.settings_config.get("env") {
                    if let Some(config_obj) = final_config.as_object_mut() {
                        // 获取或创建 env 对象
                        let env = config_obj.entry("env").or_insert(serde_json::json!({}));
                        if let Some(env_obj) = env.as_object_mut() {
                            // 只更新两个特定字段
                            if let Some(token) = provider_env.get("ANTHROPIC_AUTH_TOKEN") {
                                env_obj.insert("ANTHROPIC_AUTH_TOKEN".to_string(), token.clone());
                            }
                            if let Some(base_url) = provider_env.get("ANTHROPIC_BASE_URL") {
                                env_obj.insert("ANTHROPIC_BASE_URL".to_string(), base_url.clone());
                            }
                        }
                    }
                }

                // 写入合并后的配置
                write_json_file(&settings_path, &final_config)?;
            }
            AppType::Codex => {
                let auth = provider
                    .settings_config
                    .get("auth")
                    .ok_or_else(|| "目标供应商缺少 auth 配置".to_string())?;
                let cfg_text = provider
                    .settings_config
                    .get("config")
                    .and_then(|v| v.as_str());
                crate::codex_config::write_codex_live_atomic(auth, cfg_text)?;
            }
            AppType::Droid => {
                // Droid: 设置环境变量
                let api_key = provider
                    .settings_config
                    .get("apiKey")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Droid 配置缺少 apiKey 字段".to_string())?;
                crate::droid_config::set_factory_api_key_env(api_key)?;
            }
        }
    }

    // 更新内存并保存
    {
        let mut config = state
            .config
            .lock()
            .map_err(|e| format!("获取锁失败: {}", e))?;
        let manager = config
            .get_manager_mut(&app_type)
            .ok_or_else(|| format!("应用类型不存在: {:?}", app_type))?;
        manager
            .providers
            .insert(provider.id.clone(), provider.clone());
    }
    state.save()?;

    Ok(true)
}

/// 删除供应商
#[tauri::command]
pub async fn delete_provider(
    state: State<'_, AppState>,
    app_type: Option<AppType>,
    app: Option<String>,
    appType: Option<String>,
    id: String,
) -> Result<bool, String> {
    let app_type = app_type
        .or_else(|| app.as_deref().map(|s| s.into()))
        .or_else(|| appType.as_deref().map(|s| s.into()))
        .unwrap_or(AppType::Claude);

    let mut config = state
        .config
        .lock()
        .map_err(|e| format!("获取锁失败: {}", e))?;

    let manager = config
        .get_manager_mut(&app_type)
        .ok_or_else(|| format!("应用类型不存在: {:?}", app_type))?;

    // 检查是否为当前供应商
    if manager.current == id {
        return Err("不能删除当前正在使用的供应商".to_string());
    }

    // 获取供应商信息
    let provider = manager
        .providers
        .get(&id)
        .ok_or_else(|| format!("供应商不存在: {}", id))?
        .clone();

    // 删除配置文件
    match app_type {
        AppType::Codex => {
            codex_config::delete_codex_provider_config(&id, &provider.name)?;
        }
        AppType::Claude => {
            use crate::config::{delete_file, get_provider_config_path};
            // 兼容历史两种命名：settings-{name}.json 与 settings-{id}.json
            let by_name = get_provider_config_path(&id, Some(&provider.name));
            let by_id = get_provider_config_path(&id, None);
            delete_file(&by_name)?;
            delete_file(&by_id)?;
        }
        AppType::Droid => {
            // Droid 配置存储在主配置文件中，不需要删除额外文件
        }
    }

    // 从管理器删除
    manager.providers.remove(&id);

    // 保存配置
    drop(config); // 释放锁
    state.save()?;

    Ok(true)
}

/// 切换供应商
#[tauri::command]
pub async fn switch_provider(
    state: State<'_, AppState>,
    app_type: Option<AppType>,
    app: Option<String>,
    appType: Option<String>,
    id: String,
) -> Result<bool, String> {
    let app_type = app_type
        .or_else(|| app.as_deref().map(|s| s.into()))
        .or_else(|| appType.as_deref().map(|s| s.into()))
        .unwrap_or(AppType::Claude);

    let mut config = state
        .config
        .lock()
        .map_err(|e| format!("获取锁失败: {}", e))?;

    let manager = config
        .get_manager_mut(&app_type)
        .ok_or_else(|| format!("应用类型不存在: {:?}", app_type))?;

    // 检查供应商是否存在
    let provider = manager
        .providers
        .get(&id)
        .ok_or_else(|| format!("供应商不存在: {}", id))?
        .clone();

    // SSOT 切换：先回填 live 配置到当前供应商，然后从内存写入目标主配置
    match app_type {
        AppType::Codex => {
            use serde_json::Value;

            // 回填：读取 live（auth.json + config.toml）写回当前供应商 settings_config
            if !manager.current.is_empty() {
                let auth_path = codex_config::get_codex_auth_path();
                let config_path = codex_config::get_codex_config_path();
                if auth_path.exists() {
                    let auth: Value = crate::config::read_json_file(&auth_path)?;
                    let config_str = if config_path.exists() {
                        std::fs::read_to_string(&config_path)
                            .map_err(|e| format!("读取 config.toml 失败: {}", e))?
                    } else {
                        String::new()
                    };

                    let live = serde_json::json!({
                        "auth": auth,
                        "config": config_str,
                    });

                    if let Some(cur) = manager.providers.get_mut(&manager.current) {
                        cur.settings_config = live;
                    }
                }
            }

            // 切换：从目标供应商 settings_config 写入主配置（Codex 双文件原子+回滚）
            let auth = provider
                .settings_config
                .get("auth")
                .ok_or_else(|| "目标供应商缺少 auth 配置".to_string())?;
            let cfg_text = provider
                .settings_config
                .get("config")
                .and_then(|v| v.as_str());
            crate::codex_config::write_codex_live_atomic(auth, cfg_text)?;
        }
        AppType::Claude => {
            use crate::config::{read_json_file, write_json_file};

            let settings_path = get_claude_settings_path();

            // 回填：回填 env 字段到当前供应商（包含模型配置）
            if settings_path.exists() && !manager.current.is_empty() {
                if let Ok(live) = read_json_file::<serde_json::Value>(&settings_path) {
                    if let Some(cur) = manager.providers.get_mut(&manager.current) {
                        // 提取并保存完整的 env 字段（包括模型配置）
                        if let Some(env) = live.get("env") {
                            cur.settings_config = serde_json::json!({
                                "env": env.clone()
                            });
                        }
                    }
                }
            }

            // 切换：读取现有配置，只更新 env 字段，保留其他用户自定义配置
            if let Some(parent) = settings_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {}", e))?;
            }

            // 读取现有配置（如果存在）
            let mut final_config = if settings_path.exists() {
                read_json_file::<serde_json::Value>(&settings_path).unwrap_or(serde_json::json!({}))
            } else {
                serde_json::json!({})
            };

            // 更新 env 中的所有配置字段（支持任意环境变量）
            if let Some(provider_env) = provider.settings_config.get("env") {
                if let Some(config_obj) = final_config.as_object_mut() {
                    // 获取或创建 env 对象
                    let env = config_obj.entry("env").or_insert(serde_json::json!({}));
                    if let Some(env_obj) = env.as_object_mut() {
                        // 清除旧供应商的特有变量（避免配置污染）
                        if !manager.current.is_empty() {
                            if let Some(old_provider) = manager.providers.get(&manager.current) {
                                if let Some(old_env) = old_provider.settings_config.get("env") {
                                    if let Some(old_env_obj) = old_env.as_object() {
                                        // 删除旧供应商的所有环境变量
                                        for key in old_env_obj.keys() {
                                            env_obj.remove(key);
                                        }
                                    }
                                }
                            }
                        }

                        // 动态遍历新供应商的所有环境变量并添加
                        if let Some(provider_env_obj) = provider_env.as_object() {
                            for (key, value) in provider_env_obj {
                                env_obj.insert(key.clone(), value.clone());
                            }
                        }
                    }
                }
            }

            // 写入合并后的配置
            write_json_file(&settings_path, &final_config)?;
        }
        AppType::Droid => {
            // Droid: 设置系统环境变量
            let api_key = provider
                .settings_config
                .get("apiKey")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "目标供应商缺少 apiKey 配置".to_string())?;
            
            crate::droid_config::set_factory_api_key_env(api_key)?;
        }
    }

    // 更新当前供应商
    manager.current = id;

    log::info!("成功切换到供应商: {}", provider.name);

    // 保存配置
    drop(config); // 释放锁
    state.save()?;

    Ok(true)
}

/// 停用当前供应商（清空 env 字段）
#[tauri::command]
pub async fn disable_current_provider(
    state: State<'_, AppState>,
    app_type: Option<AppType>,
    app: Option<String>,
    appType: Option<String>,
) -> Result<bool, String> {
    let app_type = app_type
        .or_else(|| app.as_deref().map(|s| s.into()))
        .or_else(|| appType.as_deref().map(|s| s.into()))
        .unwrap_or(AppType::Claude);

    // 仅支持 Claude 和 Droid（Codex 需要 auth.json 必须有内容）
    if app_type != AppType::Claude && app_type != AppType::Droid {
        return Err("停用功能仅支持 Claude Code 和 Droid".to_string());
    }

    let mut config = state
        .config
        .lock()
        .map_err(|e| format!("获取锁失败: {}", e))?;

    let manager = config
        .get_manager_mut(&app_type)
        .ok_or_else(|| format!("应用类型不存在: {:?}", app_type))?;

    match app_type {
        AppType::Claude => {
            use crate::config::{read_json_file, write_json_file};
            let settings_path = get_claude_settings_path();

            // 读取现有配置
            let mut final_config = if settings_path.exists() {
                read_json_file::<serde_json::Value>(&settings_path).unwrap_or(serde_json::json!({}))
            } else {
                serde_json::json!({})
            };

            // 清空 env 字段（设置为空对象）
            if let Some(config_obj) = final_config.as_object_mut() {
                config_obj.insert("env".to_string(), serde_json::json!({}));
            }

            // 写入配置
            if let Some(parent) = settings_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {}", e))?;
            }
            write_json_file(&settings_path, &final_config)?;

            log::info!("已停用 Claude 供应商，env 字段已清空");
        }
        AppType::Droid => {
            // 清除系统环境变量
            crate::droid_config::clear_factory_api_key_env()?;
            log::info!("已停用 Droid 供应商，环境变量已清除");
        }
        _ => {}
    }

    // 清空当前供应商
    manager.current = String::new();

    // 保存配置
    drop(config); // 释放锁
    state.save()?;

    Ok(true)
}

/// 快速切换当前供应商的 API 地址（仅 Claude）
#[tauri::command]
pub async fn switch_provider_url(state: State<'_, AppState>, url: String) -> Result<bool, String> {
    use crate::config::{read_json_file, write_json_file};

    let mut config = state
        .config
        .lock()
        .map_err(|e| format!("获取锁失败: {}", e))?;

    let manager = config
        .get_manager_mut(&AppType::Claude)
        .ok_or_else(|| "Claude 应用类型不存在".to_string())?;

    // 获取当前供应商
    if manager.current.is_empty() {
        return Err("当前没有激活的供应商".to_string());
    }

    let provider = manager
        .providers
        .get(&manager.current)
        .ok_or_else(|| "当前供应商不存在".to_string())?
        .clone();

    // 验证 URL 是否在备选列表中
    if let Some(ref alt_urls) = provider.alternative_urls {
        if !alt_urls.contains(&url) {
            return Err(format!("地址 {} 不在供应商的备选列表中", url));
        }
    } else {
        return Err("当前供应商未配置备选地址".to_string());
    }

    let settings_path = get_claude_settings_path();

    // 读取现有配置
    let mut final_config = if settings_path.exists() {
        read_json_file::<serde_json::Value>(&settings_path).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    // 只更新 env.ANTHROPIC_BASE_URL
    if let Some(config_obj) = final_config.as_object_mut() {
        let env = config_obj.entry("env").or_insert(serde_json::json!({}));
        if let Some(env_obj) = env.as_object_mut() {
            env_obj.insert(
                "ANTHROPIC_BASE_URL".to_string(),
                serde_json::Value::String(url.clone()),
            );
        }
    }

    // 写入配置
    write_json_file(&settings_path, &final_config)?;

    // 同时更新内存中的供应商配置
    if let Some(cur) = manager.providers.get_mut(&manager.current) {
        if let Some(provider_env) = cur.settings_config.get_mut("env") {
            if let Some(env_obj) = provider_env.as_object_mut() {
                env_obj.insert(
                    "ANTHROPIC_BASE_URL".to_string(),
                    serde_json::Value::String(url.clone()),
                );
            }
        }
    }

    log::info!("已切换当前供应商的 API 地址到: {}", url);

    // 保存配置
    drop(config);
    state.save()?;

    Ok(true)
}

/// 导入当前配置为默认供应商
#[tauri::command]
pub async fn import_default_config(
    state: State<'_, AppState>,
    app_type: Option<AppType>,
    app: Option<String>,
    appType: Option<String>,
) -> Result<bool, String> {
    let app_type = app_type
        .or_else(|| app.as_deref().map(|s| s.into()))
        .or_else(|| appType.as_deref().map(|s| s.into()))
        .unwrap_or(AppType::Claude);

    // 仅当 providers 为空时才从 live 导入一条默认项
    {
        let config = state
            .config
            .lock()
            .map_err(|e| format!("获取锁失败: {}", e))?;

        if let Some(manager) = config.get_manager(&app_type) {
            if !manager.get_all_providers().is_empty() {
                return Ok(true);
            }
        }
    }

    // 根据应用类型导入配置
    // 读取当前主配置为默认供应商（不再写入副本文件）
    let settings_config = match app_type {
        AppType::Codex => {
            let auth_path = codex_config::get_codex_auth_path();
            if !auth_path.exists() {
                return Err("Codex 配置文件不存在".to_string());
            }
            let auth: serde_json::Value =
                crate::config::read_json_file::<serde_json::Value>(&auth_path)?;
            let config_str = match crate::codex_config::read_and_validate_codex_config_text() {
                Ok(s) => s,
                Err(e) => return Err(e),
            };
            serde_json::json!({ "auth": auth, "config": config_str })
        }
        AppType::Claude => {
            let settings_path = get_claude_settings_path();
            if !settings_path.exists() {
                return Err("Claude Code 配置文件不存在".to_string());
            }
            let full_config = crate::config::read_json_file::<serde_json::Value>(&settings_path)?;

            // 只提取 env 字段
            let env = full_config
                .get("env")
                .cloned()
                .unwrap_or(serde_json::json!({}));
            serde_json::json!({ "env": env })
        }
        AppType::Droid => {
            return Err("Droid 不支持自动导入默认配置，请手动添加 API Key".to_string());
        }
    };

    // 创建默认供应商（仅首次初始化）
    let provider = Provider::with_id(
        "default".to_string(),
        "default".to_string(),
        settings_config,
        None,
    );

    // 添加到管理器
    let mut config = state
        .config
        .lock()
        .map_err(|e| format!("获取锁失败: {}", e))?;

    let manager = config
        .get_manager_mut(&app_type)
        .ok_or_else(|| format!("应用类型不存在: {:?}", app_type))?;

    manager.providers.insert(provider.id.clone(), provider);
    // 设置当前供应商为默认项
    manager.current = "default".to_string();

    // 保存配置
    drop(config); // 释放锁
    state.save()?;

    Ok(true)
}

/// 获取 Claude Code 配置状态
#[tauri::command]
pub async fn get_claude_config_status() -> Result<ConfigStatus, String> {
    Ok(crate::config::get_claude_config_status())
}

/// 获取应用配置状态（通用）
/// 兼容两种参数：`app_type`（推荐）或 `app`（字符串）
#[tauri::command]
pub async fn get_config_status(
    app_type: Option<AppType>,
    app: Option<String>,
    appType: Option<String>,
) -> Result<ConfigStatus, String> {
    let app = app_type
        .or_else(|| app.as_deref().map(|s| s.into()))
        .or_else(|| appType.as_deref().map(|s| s.into()))
        .unwrap_or(AppType::Claude);

    match app {
        AppType::Claude => Ok(crate::config::get_claude_config_status()),
        AppType::Codex => {
            use crate::codex_config::{get_codex_auth_path, get_codex_config_dir};
            let auth_path = get_codex_auth_path();

            // 放宽：只要 auth.json 存在即可认为已配置；config.toml 允许为空
            let exists = auth_path.exists();
            let path = get_codex_config_dir().to_string_lossy().to_string();

            Ok(ConfigStatus { exists, path })
        }
        AppType::Droid => {
            use crate::droid_config::get_droid_config_dir;
            let config_dir = get_droid_config_dir();
            
            // Droid 配置目录存在即认为已配置
            let exists = config_dir.exists();
            let path = config_dir.to_string_lossy().to_string();

            Ok(ConfigStatus { exists, path })
        }
    }
}

/// 获取 Claude Code 配置文件路径
#[tauri::command]
pub async fn get_claude_code_config_path() -> Result<String, String> {
    Ok(get_claude_settings_path().to_string_lossy().to_string())
}

/// 获取当前 Claude settings.json 的完整内容
#[tauri::command]
pub async fn get_current_claude_settings() -> Result<serde_json::Value, String> {
    let settings_path = get_claude_settings_path();
    if !settings_path.exists() {
        return Err("Claude Code 配置文件不存在".to_string());
    }

    crate::config::read_json_file::<serde_json::Value>(&settings_path)
}

/// 同步当前供应商配置（从 live settings.json 回填）
#[tauri::command]
pub async fn sync_current_provider_config(
    state: State<'_, AppState>,
    app_type: Option<AppType>,
    app: Option<String>,
    appType: Option<String>,
) -> Result<bool, String> {
    let app_type = app_type
        .or_else(|| app.as_deref().map(|s| s.into()))
        .or_else(|| appType.as_deref().map(|s| s.into()))
        .unwrap_or(AppType::Claude);

    match app_type {
        AppType::Claude => {
            let settings_path = get_claude_settings_path();
            if !settings_path.exists() {
                return Err("Claude Code 配置文件不存在".to_string());
            }

            // 读取 live settings.json
            let live_config = crate::config::read_json_file::<serde_json::Value>(&settings_path)?;

            // 更新当前供应商的配置
            let mut config = state
                .config
                .lock()
                .map_err(|e| format!("获取锁失败: {}", e))?;

            let manager = config
                .get_manager_mut(&app_type)
                .ok_or_else(|| format!("应用类型不存在: {:?}", app_type))?;

            if !manager.current.is_empty() {
                if let Some(current_provider) = manager.providers.get_mut(&manager.current) {
                    // 只提取并同步 env 字段
                    let env = live_config
                        .get("env")
                        .cloned()
                        .unwrap_or(serde_json::json!({}));
                    current_provider.settings_config = serde_json::json!({ "env": env });
                    log::info!("已同步当前供应商 '{}' 的 env 配置", current_provider.name);
                }
            }

            // 保存配置
            drop(config);
            state.save()?;
            Ok(true)
        }
        AppType::Codex => {
            // Codex 的同步逻辑（如果需要的话）
            Ok(true)
        }
        AppType::Droid => {
            // Droid 不需要同步逻辑，配置存储在主配置文件中
            Ok(true)
        }
    }
}

/// 获取当前生效的配置目录
#[tauri::command]
pub async fn get_config_dir(
    app_type: Option<AppType>,
    app: Option<String>,
    appType: Option<String>,
) -> Result<String, String> {
    let app = app_type
        .or_else(|| app.as_deref().map(|s| s.into()))
        .or_else(|| appType.as_deref().map(|s| s.into()))
        .unwrap_or(AppType::Claude);

    let dir = match app {
        AppType::Claude => config::get_claude_config_dir(),
        AppType::Codex => codex_config::get_codex_config_dir(),
        AppType::Droid => crate::droid_config::get_droid_config_dir(),
    };

    Ok(dir.to_string_lossy().to_string())
}

/// 打开配置文件夹
/// 兼容两种参数：`app_type`（推荐）或 `app`（字符串）
#[tauri::command]
pub async fn open_config_folder(
    handle: tauri::AppHandle,
    app_type: Option<AppType>,
    app: Option<String>,
    appType: Option<String>,
) -> Result<bool, String> {
    let app_type = app_type
        .or_else(|| app.as_deref().map(|s| s.into()))
        .or_else(|| appType.as_deref().map(|s| s.into()))
        .unwrap_or(AppType::Claude);

    let config_dir = match app_type {
        AppType::Claude => crate::config::get_claude_config_dir(),
        AppType::Codex => crate::codex_config::get_codex_config_dir(),
        AppType::Droid => crate::droid_config::get_droid_config_dir(),
    };

    // 确保目录存在
    if !config_dir.exists() {
        std::fs::create_dir_all(&config_dir).map_err(|e| format!("创建目录失败: {}", e))?;
    }

    // 使用 opener 插件打开文件夹
    handle
        .opener()
        .open_path(config_dir.to_string_lossy().to_string(), None::<String>)
        .map_err(|e| format!("打开文件夹失败: {}", e))?;

    Ok(true)
}

/// 弹出系统目录选择器并返回用户选择的路径
#[tauri::command]
pub async fn pick_directory(
    app: tauri::AppHandle,
    default_path: Option<String>,
) -> Result<Option<String>, String> {
    let initial = default_path
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty());

    let result = tauri::async_runtime::spawn_blocking(move || {
        let mut builder = app.dialog().file();
        if let Some(path) = initial {
            builder = builder.set_directory(path);
        }
        builder.blocking_pick_folder()
    })
    .await
    .map_err(|e| format!("弹出目录选择器失败: {}", e))?;

    match result {
        Some(file_path) => {
            let resolved = file_path
                .simplified()
                .into_path()
                .map_err(|e| format!("解析选择的目录失败: {}", e))?;
            Ok(Some(resolved.to_string_lossy().to_string()))
        }
        None => Ok(None),
    }
}

/// 打开外部链接
#[tauri::command]
pub async fn open_external(app: tauri::AppHandle, url: String) -> Result<bool, String> {
    // 规范化 URL，缺少协议时默认加 https://
    let url = if url.starts_with("http://") || url.starts_with("https://") {
        url
    } else {
        format!("https://{}", url)
    };

    // 使用 opener 插件打开链接
    app.opener()
        .open_url(&url, None::<String>)
        .map_err(|e| format!("打开链接失败: {}", e))?;

    Ok(true)
}

/// 获取应用配置文件路径
#[tauri::command]
pub async fn get_app_config_path() -> Result<String, String> {
    use crate::config::get_app_config_path;

    let config_path = get_app_config_path();
    Ok(config_path.to_string_lossy().to_string())
}

/// 打开应用配置文件夹
#[tauri::command]
pub async fn open_app_config_folder(handle: tauri::AppHandle) -> Result<bool, String> {
    use crate::config::get_app_config_dir;

    let config_dir = get_app_config_dir();

    // 确保目录存在
    if !config_dir.exists() {
        std::fs::create_dir_all(&config_dir).map_err(|e| format!("创建目录失败: {}", e))?;
    }

    // 使用 opener 插件打开文件夹
    handle
        .opener()
        .open_path(config_dir.to_string_lossy().to_string(), None::<String>)
        .map_err(|e| format!("打开文件夹失败: {}", e))?;

    Ok(true)
}

/// 获取设置
#[tauri::command]
pub async fn get_settings() -> Result<crate::settings::AppSettings, String> {
    Ok(crate::settings::get_settings())
}

/// 保存设置
#[tauri::command]
pub async fn save_settings(
    app: tauri::AppHandle,
    settings: crate::settings::AppSettings,
) -> Result<bool, String> {
    // 处理开机自启动设置
    let autostart_manager = app.autolaunch();
    let is_enabled = autostart_manager
        .is_enabled()
        .map_err(|e| format!("检查自启动状态失败: {}", e))?;

    if settings.auto_start && !is_enabled {
        // 需要启用自启动
        autostart_manager
            .enable()
            .map_err(|e| format!("启用开机自启动失败: {}", e))?;
        log::info!("已启用开机自启动");
    } else if !settings.auto_start && is_enabled {
        // 需要禁用自启动
        autostart_manager
            .disable()
            .map_err(|e| format!("禁用开机自启动失败: {}", e))?;
        log::info!("已禁用开机自启动");
    }

    // 处理全局快捷键设置
    let old_settings = crate::settings::get_settings();
    let old_shortcut = old_settings.global_shortcut.clone();
    let new_shortcut = settings.global_shortcut.clone();

    // 如果快捷键发生变化，需要重新注册
    if old_shortcut != new_shortcut {
        // 先注销旧的快捷键
        if let Some(old) = old_shortcut {
            if !old.is_empty() {
                if let Ok(shortcut) = Shortcut::from_str(&old) {
                    let _ = app.global_shortcut().unregister(shortcut);
                }
            }
        }

        // 注册新的快捷键
        if let Some(new) = &new_shortcut {
            if !new.is_empty() {
                let shortcut = Shortcut::from_str(new)
                    .map_err(|e| format!("快捷键格式无效: {}", e))?;
                let app_handle = app.clone();
                if let Err(e) = app.global_shortcut().on_shortcut(shortcut, move |_app, _shortcut, _event| {
                    // 使用防抖机制避免快速重复触发
                    crate::toggle_main_window(&app_handle);
                }) {
                    log::error!("注册全局快捷键失败: {}", e);
                    return Err(format!("注册全局快捷键失败: {}", e));
                } else {
                    log::info!("已注册全局快捷键: {}", new);
                }
            }
        }
    }

    crate::settings::update_settings(settings)?;
    Ok(true)
}

/// 获取开机自启动状态
#[tauri::command]
pub async fn get_autostart_status(app: tauri::AppHandle) -> Result<bool, String> {
    let autostart_manager = app.autolaunch();
    autostart_manager
        .is_enabled()
        .map_err(|e| format!("检查自启动状态失败: {}", e))
}

/// 设置开机自启动
#[tauri::command]
pub async fn set_autostart(app: tauri::AppHandle, enable: bool) -> Result<bool, String> {
    let autostart_manager = app.autolaunch();

    log::info!("尝试{}开机自启动", if enable { "启用" } else { "禁用" });

    if enable {
        match autostart_manager.enable() {
            Ok(_) => {
                log::info!("✅ 已启用开机自启动");
                // 验证是否真的启用了
                match autostart_manager.is_enabled() {
                    Ok(true) => log::info!("验证成功：开机自启动已启用"),
                    Ok(false) => log::warn!("⚠️ 警告：启用命令执行成功，但验证显示未启用"),
                    Err(e) => log::warn!("⚠️ 无法验证自启动状态: {}", e),
                }
            }
            Err(e) => {
                log::error!("❌ 启用开机自启动失败: {}", e);
                return Err(format!("启用开机自启动失败: {}", e));
            }
        }
    } else {
        match autostart_manager.disable() {
            Ok(_) => log::info!("✅ 已禁用开机自启动"),
            Err(e) => {
                log::error!("❌ 禁用开机自启动失败: {}", e);
                return Err(format!("禁用开机自启动失败: {}", e));
            }
        }
    }

    Ok(true)
}

/// 注册全局快捷键
#[tauri::command]
pub async fn register_global_shortcut(
    app: tauri::AppHandle,
    shortcut_str: String,
) -> Result<bool, String> {
    if shortcut_str.is_empty() {
        return Err("快捷键不能为空".to_string());
    }

    let shortcut = Shortcut::from_str(&shortcut_str)
        .map_err(|e| format!("快捷键格式无效: {}", e))?;

    let app_handle = app.clone();
    app.global_shortcut()
        .on_shortcut(shortcut, move |_app, _shortcut, _event| {
            // 使用防抖机制避免快速重复触发
            crate::toggle_main_window(&app_handle);
        })
        .map_err(|e| format!("注册全局快捷键失败: {}", e))?;

    log::info!("已注册全局快捷键: {}", shortcut_str);
    Ok(true)
}

/// 注销全局快捷键
#[tauri::command]
pub async fn unregister_global_shortcut(
    app: tauri::AppHandle,
    shortcut_str: String,
) -> Result<bool, String> {
    if shortcut_str.is_empty() {
        return Ok(true);
    }

    let shortcut = Shortcut::from_str(&shortcut_str)
        .map_err(|e| format!("快捷键格式无效: {}", e))?;

    app.global_shortcut()
        .unregister(shortcut)
        .map_err(|e| format!("注销全局快捷键失败: {}", e))?;

    log::info!("已注销全局快捷键: {}", shortcut_str);
    Ok(true)
}

/// 验证快捷键是否有效
#[tauri::command]
pub async fn validate_global_shortcut(
    app: tauri::AppHandle,
    shortcut_str: String,
) -> Result<bool, String> {
    if shortcut_str.is_empty() {
        return Ok(true); // 空字符串视为有效（表示不使用快捷键）
    }

    // 尝试解析快捷键来验证其格式是否正确
    match Shortcut::from_str(&shortcut_str) {
        Ok(shortcut) => {
            // 尝试注册快捷键来验证其是否可用
            let result = app.global_shortcut().on_shortcut(shortcut.clone(), |_app, _shortcut, _event| {
                // 空回调，仅用于验证
            });

            match result {
                Ok(_) => {
                    // 验证成功后立即注销
                    let _ = app.global_shortcut().unregister(shortcut);
                    Ok(true)
                }
                Err(e) => Err(format!("快捷键注册失败: {}", e)),
            }
        }
        Err(e) => Err(format!("快捷键格式无效: {}", e)),
    }
}

/// 检查更新
#[tauri::command]
pub async fn check_for_updates(handle: tauri::AppHandle) -> Result<bool, String> {
    // 打开 GitHub releases 页面
    handle
        .opener()
        .open_url(
            "https://github.com/farion1231/cc-switch/releases/latest",
            None::<String>,
        )
        .map_err(|e| format!("打开更新页面失败: {}", e))?;

    Ok(true)
}

/// 判断是否为便携版（绿色版）运行
#[tauri::command]
pub async fn is_portable_mode() -> Result<bool, String> {
    let exe_path = std::env::current_exe().map_err(|e| format!("获取可执行路径失败: {}", e))?;
    if let Some(dir) = exe_path.parent() {
        Ok(dir.join("portable.ini").is_file())
    } else {
        Ok(false)
    }
}

/// VS Code: 获取用户 settings.json 状态
#[tauri::command]
pub async fn get_vscode_settings_status() -> Result<ConfigStatus, String> {
    if let Some(p) = vscode::find_existing_settings() {
        Ok(ConfigStatus {
            exists: true,
            path: p.to_string_lossy().to_string(),
        })
    } else {
        // 默认返回 macOS 稳定版路径（或其他平台首选项的第一个候选），但标记不存在
        let preferred = vscode::candidate_settings_paths().into_iter().next();
        Ok(ConfigStatus {
            exists: false,
            path: preferred.unwrap_or_default().to_string_lossy().to_string(),
        })
    }
}

/// VS Code: 读取 settings.json 文本（仅当文件存在）
#[tauri::command]
pub async fn read_vscode_settings() -> Result<String, String> {
    if let Some(p) = vscode::find_existing_settings() {
        std::fs::read_to_string(&p).map_err(|e| format!("读取 VS Code 设置失败: {}", e))
    } else {
        Err("未找到 VS Code 用户设置文件".to_string())
    }
}

/// VS Code: 写入 settings.json 文本（仅当文件存在；不自动创建）
#[tauri::command]
pub async fn write_vscode_settings(content: String) -> Result<bool, String> {
    if let Some(p) = vscode::find_existing_settings() {
        config::write_text_file(&p, &content)?;
        Ok(true)
    } else {
        Err("未找到 VS Code 用户设置文件".to_string())
    }
}

/// Claude 插件：获取 ~/.claude/config.json 状态
#[tauri::command]
pub async fn get_claude_plugin_status() -> Result<ConfigStatus, String> {
    match claude_plugin::claude_config_status() {
        Ok((exists, path)) => Ok(ConfigStatus {
            exists,
            path: path.to_string_lossy().to_string(),
        }),
        Err(err) => Err(err),
    }
}

/// Claude 插件：读取配置内容（若不存在返回 Ok(None)）
#[tauri::command]
pub async fn read_claude_plugin_config() -> Result<Option<String>, String> {
    claude_plugin::read_claude_config()
}

/// Claude 插件：写入/清除固定配置
#[tauri::command]
pub async fn apply_claude_plugin_config(official: bool) -> Result<bool, String> {
    if official {
        claude_plugin::clear_claude_config()
    } else {
        claude_plugin::write_claude_config()
    }
}

/// Claude 插件：检测是否已写入目标配置
#[tauri::command]
pub async fn is_claude_plugin_applied() -> Result<bool, String> {
    claude_plugin::is_claude_config_applied()
}

// =====================
// 新：集中以 config.json 为 SSOT 的 MCP 配置命令
// =====================

#[derive(serde::Serialize)]
pub struct McpConfigResponse {
    pub config_path: String,
    pub servers: std::collections::HashMap<String, serde_json::Value>,
}

/// 获取 MCP 配置（来自 ~/.cc-switch/config.json）
#[tauri::command]
pub async fn get_mcp_config(
    state: State<'_, AppState>,
    app: Option<String>,
) -> Result<McpConfigResponse, String> {
    let config_path = crate::config::get_app_config_path()
        .to_string_lossy()
        .to_string();
    let mut cfg = state
        .config
        .lock()
        .map_err(|e| format!("获取锁失败: {}", e))?;
    let app_ty = crate::app_config::AppType::from(app.as_deref().unwrap_or("claude"));
    let (servers, normalized) = crate::mcp::get_servers_snapshot_for(&mut cfg, &app_ty);
    let need_save = normalized > 0;
    drop(cfg);
    if need_save {
        state.save()?;
    }
    Ok(McpConfigResponse {
        config_path,
        servers,
    })
}

/// 在 config.json 中新增或更新一个 MCP 服务器定义
#[tauri::command]
pub async fn upsert_mcp_server_in_config(
    state: State<'_, AppState>,
    app: Option<String>,
    id: String,
    spec: serde_json::Value,
) -> Result<bool, String> {
    let mut cfg = state
        .config
        .lock()
        .map_err(|e| format!("获取锁失败: {}", e))?;
    let app_ty = crate::app_config::AppType::from(app.as_deref().unwrap_or("claude"));

    // v3.5.1: 检查 MCP 是否已启用（编辑前）
    let was_enabled = crate::mcp::is_mcp_enabled(&cfg, &app_ty, &id);

    let changed = crate::mcp::upsert_in_config_for(&mut cfg, &app_ty, &id, spec)?;

    // v3.5.1: 如果 MCP 已启用，自动同步到 live 配置
    if was_enabled {
        match app_ty {
            crate::app_config::AppType::Claude => {
                crate::mcp::sync_enabled_to_claude(&cfg)?;
            }
            crate::app_config::AppType::Codex => {
                crate::mcp::sync_enabled_to_codex(&cfg)?;
            }
            crate::app_config::AppType::Droid => {
                crate::mcp::sync_enabled_to_droid(&cfg)?;
            }
        }
        log::info!("已自动同步已启用的 MCP '{}' 到 live 配置", id);
    }

    drop(cfg);
    state.save()?;
    Ok(changed)
}

/// 在 config.json 中删除一个 MCP 服务器定义
#[tauri::command]
pub async fn delete_mcp_server_in_config(
    state: State<'_, AppState>,
    app: Option<String>,
    id: String,
) -> Result<bool, String> {
    let mut cfg = state
        .config
        .lock()
        .map_err(|e| format!("获取锁失败: {}", e))?;
    let app_ty = crate::app_config::AppType::from(app.as_deref().unwrap_or("claude"));
    let existed = crate::mcp::delete_in_config_for(&mut cfg, &app_ty, &id)?;
    drop(cfg);
    state.save()?;
    // 若删除的是 Claude/Codex 客户端的条目，则同步一次，确保启用项从对应 live 配置中移除
    let cfg2 = state
        .config
        .lock()
        .map_err(|e| format!("获取锁失败: {}", e))?;
    match app_ty {
        crate::app_config::AppType::Claude => crate::mcp::sync_enabled_to_claude(&cfg2)?,
        crate::app_config::AppType::Codex => crate::mcp::sync_enabled_to_codex(&cfg2)?,
        crate::app_config::AppType::Droid => {
            // Droid 暂不支持 MCP 同步
        }
    }
    Ok(existed)
}

/// 设置启用状态并同步到 ~/.claude.json
#[tauri::command]
pub async fn set_mcp_enabled(
    state: State<'_, AppState>,
    app: Option<String>,
    id: String,
    enabled: bool,
) -> Result<bool, String> {
    let mut cfg = state
        .config
        .lock()
        .map_err(|e| format!("获取锁失败: {}", e))?;
    let app_ty = crate::app_config::AppType::from(app.as_deref().unwrap_or("claude"));
    let changed = crate::mcp::set_enabled_and_sync_for(&mut cfg, &app_ty, &id, enabled)?;
    drop(cfg);
    state.save()?;
    Ok(changed)
}

/// 手动同步：将启用的 MCP 投影到 ~/.claude.json（不更改 config.json）
#[tauri::command]
pub async fn sync_enabled_mcp_to_claude(state: State<'_, AppState>) -> Result<bool, String> {
    let mut cfg = state
        .config
        .lock()
        .map_err(|e| format!("获取锁失败: {}", e))?;
    let normalized = crate::mcp::normalize_servers_for(&mut cfg, &AppType::Claude);
    crate::mcp::sync_enabled_to_claude(&cfg)?;
    let need_save = normalized > 0;
    drop(cfg);
    if need_save {
        state.save()?;
    }
    Ok(true)
}

/// 手动同步：将启用的 MCP 投影到 ~/.codex/config.toml（不更改 config.json）
#[tauri::command]
pub async fn sync_enabled_mcp_to_codex(state: State<'_, AppState>) -> Result<bool, String> {
    let mut cfg = state
        .config
        .lock()
        .map_err(|e| format!("获取锁失败: {}", e))?;
    let normalized = crate::mcp::normalize_servers_for(&mut cfg, &AppType::Codex);
    crate::mcp::sync_enabled_to_codex(&cfg)?;
    let need_save = normalized > 0;
    drop(cfg);
    if need_save {
        state.save()?;
    }
    Ok(true)
}

/// 从 ~/.claude.json 导入 MCP 定义到 config.json，返回变更数量
#[tauri::command]
pub async fn import_mcp_from_claude(state: State<'_, AppState>) -> Result<usize, String> {
    let mut cfg = state
        .config
        .lock()
        .map_err(|e| format!("获取锁失败: {}", e))?;
    let changed = crate::mcp::import_from_claude(&mut cfg)?;
    drop(cfg);
    if changed > 0 {
        state.save()?;
    }
    Ok(changed)
}

/// 从 ~/.codex/config.toml 导入 MCP 定义到 config.json（Codex 作用域），返回变更数量
#[tauri::command]
pub async fn import_mcp_from_codex(state: State<'_, AppState>) -> Result<usize, String> {
    let mut cfg = state
        .config
        .lock()
        .map_err(|e| format!("获取锁失败: {}", e))?;
    let changed = crate::mcp::import_from_codex(&mut cfg)?;
    drop(cfg);
    if changed > 0 {
        state.save()?;
    }
    Ok(changed)
}

/// 手动同步：将启用的 MCP 投影到 ~/.factory/mcp.json（不更改 config.json）
#[tauri::command]
pub async fn sync_enabled_mcp_to_droid(state: State<'_, AppState>) -> Result<bool, String> {
    let mut cfg = state
        .config
        .lock()
        .map_err(|e| format!("获取锁失败: {}", e))?;
    let normalized = crate::mcp::normalize_servers_for(&mut cfg, &AppType::Droid);
    crate::mcp::sync_enabled_to_droid(&cfg)?;
    let need_save = normalized > 0;
    drop(cfg);
    if need_save {
        state.save()?;
    }
    Ok(true)
}

/// 从 ~/.factory/mcp.json 导入 MCP 定义到 config.json，返回变更数量
#[tauri::command]
pub async fn import_mcp_from_droid(state: State<'_, AppState>) -> Result<usize, String> {
    let mut cfg = state
        .config
        .lock()
        .map_err(|e| format!("获取锁失败: {}", e))?;
    let changed = crate::mcp::import_from_droid(&mut cfg)?;
    drop(cfg);
    if changed > 0 {
        state.save()?;
    }
    Ok(changed)
}

// =====================
// v3.5.1 新增：MCP 配置双端同步命令
// =====================

/// 检查目标应用是否存在同名 MCP 服务器
#[tauri::command]
pub async fn check_mcp_sync_conflict(
    state: State<'_, AppState>,
    target_app: Option<String>,
    id: String,
) -> Result<bool, String> {
    let cfg = state
        .config
        .lock()
        .map_err(|e| format!("获取锁失败: {}", e))?;
    let target_app_ty = crate::app_config::AppType::from(target_app.as_deref().unwrap_or("claude"));
    let exists = crate::mcp::check_mcp_exists_in_target_app(&cfg, &target_app_ty, &id);
    Ok(exists)
}

/// 将 MCP 服务器同步到目标应用（三向同步：Claude ↔ Codex ↔ Droid）
#[tauri::command]
pub async fn sync_mcp_to_other_app(
    state: State<'_, AppState>,
    source_app: Option<String>,
    target_app: Option<String>,
    id: String,
    overwrite: bool,
) -> Result<bool, String> {
    let mut cfg = state
        .config
        .lock()
        .map_err(|e| format!("获取锁失败: {}", e))?;
    let source_app_ty = crate::app_config::AppType::from(source_app.as_deref().unwrap_or("claude"));
    let target_app_ty = crate::app_config::AppType::from(target_app.as_deref().unwrap_or("codex"));
    let synced = crate::mcp::copy_mcp_to_target_app(&mut cfg, &source_app_ty, &target_app_ty, &id, overwrite)?;
    drop(cfg);
    if synced {
        state.save()?;
    }
    Ok(synced)
}

// =====================
// 端点测速命令
// =====================

/// 测试多个端点的响应速度
#[tauri::command]
pub async fn test_endpoints(
  urls: Vec<String>,
  timeout_secs: Option<u64>,
) -> Result<Vec<crate::speedtest::EndpointLatency>, String> {
  crate::speedtest::test_endpoints(urls, timeout_secs).await
}

// =====================
// Droid 余额查询命令
// =====================

/// 查询单个 Droid API Key 的余额
#[tauri::command]
pub async fn check_droid_balance(
    apiKey: String,
) -> Result<crate::droid_config::BalanceInfo, String> {
    crate::droid_config::check_balance(&apiKey).await
}

/// 批量查询 Droid API Keys 的余额
#[tauri::command]
pub async fn batch_check_droid_balances(
    apiKeys: Vec<String>,
) -> Result<std::collections::HashMap<String, crate::droid_config::BalanceInfo>, String> {
    crate::droid_config::batch_check_balances(apiKeys).await
}

/// 获取当前系统环境变量 Factory_API_Key 的值
#[tauri::command]
pub async fn get_factory_api_key_env() -> Result<Option<String>, String> {
    crate::droid_config::get_factory_api_key_env()
}
