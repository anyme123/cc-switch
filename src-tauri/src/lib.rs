mod app_config;
mod claude_mcp;
mod claude_plugin;
mod codex_config;
mod commands;
mod config;
mod droid_config;
mod factory_mcp;
mod mcp;
mod migration;
mod provider;
mod settings;
mod speedtest;
mod store;
mod vscode;

use std::str::FromStr;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use store::AppState;
use tauri::{
    menu::{CheckMenuItem, Menu, MenuBuilder, MenuItem, SubmenuBuilder},
    tray::{TrayIconBuilder, TrayIconEvent},
};
#[cfg(target_os = "macos")]
use tauri::{ActivationPolicy, RunEvent};
use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

/// 创建动态托盘菜单
fn create_tray_menu(
    app: &tauri::AppHandle,
    app_state: &AppState,
) -> Result<Menu<tauri::Wry>, String> {
    let config = app_state
        .config
        .lock()
        .map_err(|e| format!("获取锁失败: {}", e))?;

    let mut menu_builder = MenuBuilder::new(app);

    // 顶部：打开主界面
    let show_main_item = MenuItem::with_id(app, "show_main", "打开主界面", true, None::<&str>)
        .map_err(|e| format!("创建打开主界面菜单失败: {}", e))?;
    menu_builder = menu_builder.item(&show_main_item).separator();

    // 使用子菜单组织 3 大类，支持折叠
    
    // === Claude 子菜单 ===
    if let Some(claude_manager) = config.get_manager(&crate::app_config::AppType::Claude) {
        let mut claude_submenu = SubmenuBuilder::new(app, "Claude");

        if !claude_manager.providers.is_empty() {
            for (id, provider) in &claude_manager.providers {
                let is_current = claude_manager.current == *id;
                
                // 检查是否有多个端点
                let has_multiple_endpoints = provider.alternative_urls.as_ref()
                    .map(|urls| urls.len() > 1)
                    .unwrap_or(false);

                if has_multiple_endpoints {
                    // 有多个端点：创建供应商子菜单（无论是否激活）
                    let submenu_name = if is_current {
                        format!("{} ✓", &provider.name)
                    } else {
                        provider.name.clone()
                    };
                    
                    let mut provider_submenu = SubmenuBuilder::new(app, submenu_name);
                    
                    // 获取当前使用的URL（只有激活时才有意义）
                    let current_url = if is_current {
                        provider
                            .settings_config
                            .get("env")
                            .and_then(|env| env.get("ANTHROPIC_BASE_URL"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                    } else {
                        ""
                    };

                    // 添加所有端点（所有供应商的端点都可点击，实现一步到位切换）
                    if let Some(ref alt_urls) = provider.alternative_urls {
                        for url in alt_urls {
                            let is_current_url = is_current && url == current_url;
                            // 编码格式：provider_id + url
                            let encoded_url = url.replace("://", "___").replace('/', "__");
                            
                            // 所有端点都可点击：点击时自动激活供应商并切换端点
                            let url_item = CheckMenuItem::with_id(
                                app,
                                format!("claude_endpoint_{}_{}", id, encoded_url),
                                url,
                                true,
                                is_current_url,
                                None::<&str>,
                            )
                            .map_err(|e| format!("创建端点菜单项失败: {}", e))?;
                            provider_submenu = provider_submenu.item(&url_item);
                        }
                    }

                    let provider_menu = provider_submenu
                        .build()
                        .map_err(|e| format!("构建供应商子菜单失败: {}", e))?;
                    claude_submenu = claude_submenu.item(&provider_menu);
                } else {
                    // 没有多个端点：普通菜单项
                    let item = CheckMenuItem::with_id(
                        app,
                        format!("claude_{}", id),
                        &provider.name,
                        true,
                        is_current,
                        None::<&str>,
                    )
                    .map_err(|e| format!("创建菜单项失败: {}", e))?;
                    claude_submenu = claude_submenu.item(&item);
                }
            }

            // 如果有当前供应商，添加停用按钮
            if !claude_manager.current.is_empty() {
                claude_submenu = claude_submenu.separator();
                let disable_item = MenuItem::with_id(
                    app,
                    "claude_disable",
                    "停用当前供应商",
                    true,
                    None::<&str>,
                )
                .map_err(|e| format!("创建停用菜单失败: {}", e))?;
                claude_submenu = claude_submenu.item(&disable_item);
            }
        } else {
            let empty_hint = MenuItem::with_id(
                app,
                "claude_empty",
                "(无供应商)",
                false,
                None::<&str>,
            )
            .map_err(|e| format!("创建Claude空提示失败: {}", e))?;
            claude_submenu = claude_submenu.item(&empty_hint);
        }

        let claude_menu = claude_submenu
            .build()
            .map_err(|e| format!("构建Claude子菜单失败: {}", e))?;
        menu_builder = menu_builder.item(&claude_menu);
    }

    // === Codex 子菜单 ===
    if let Some(codex_manager) = config.get_manager(&crate::app_config::AppType::Codex) {
        let mut codex_submenu = SubmenuBuilder::new(app, "Codex");

        if !codex_manager.providers.is_empty() {
            for (id, provider) in &codex_manager.providers {
                let is_current = codex_manager.current == *id;
                let item = CheckMenuItem::with_id(
                    app,
                    format!("codex_{}", id),
                    &provider.name,
                    true,
                    is_current,
                    None::<&str>,
                )
                .map_err(|e| format!("创建菜单项失败: {}", e))?;
                codex_submenu = codex_submenu.item(&item);
            }
        } else {
            let empty_hint = MenuItem::with_id(
                app,
                "codex_empty",
                "(无供应商)",
                false,
                None::<&str>,
            )
            .map_err(|e| format!("创建Codex空提示失败: {}", e))?;
            codex_submenu = codex_submenu.item(&empty_hint);
        }

        let codex_menu = codex_submenu
            .build()
            .map_err(|e| format!("构建Codex子菜单失败: {}", e))?;
        menu_builder = menu_builder.item(&codex_menu);
    }

    // === Droid 子菜单 ===
    if let Some(droid_manager) = config.get_manager(&crate::app_config::AppType::Droid) {
        let mut droid_submenu = SubmenuBuilder::new(app, "Droid");

        if !droid_manager.providers.is_empty() {
            for (id, provider) in &droid_manager.providers {
                let is_current = droid_manager.current == *id;
                let item = CheckMenuItem::with_id(
                    app,
                    format!("droid_{}", id),
                    &provider.name,
                    true,
                    is_current,
                    None::<&str>,
                )
                .map_err(|e| format!("创建菜单项失败: {}", e))?;
                droid_submenu = droid_submenu.item(&item);
            }

            // 如果有当前供应商，添加停用按钮
            if !droid_manager.current.is_empty() {
                droid_submenu = droid_submenu.separator();
                let disable_item = MenuItem::with_id(
                    app,
                    "droid_disable",
                    "停用当前供应商",
                    true,
                    None::<&str>,
                )
                .map_err(|e| format!("创建停用菜单失败: {}", e))?;
                droid_submenu = droid_submenu.item(&disable_item);
            }
        } else {
            let empty_hint = MenuItem::with_id(
                app,
                "droid_empty",
                "(无供应商)",
                false,
                None::<&str>,
            )
            .map_err(|e| format!("创建Droid空提示失败: {}", e))?;
            droid_submenu = droid_submenu.item(&empty_hint);
        }

        let droid_menu = droid_submenu
            .build()
            .map_err(|e| format!("构建Droid子菜单失败: {}", e))?;
        menu_builder = menu_builder.item(&droid_menu);
    }

    // 分隔符和退出菜单
    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)
        .map_err(|e| format!("创建退出菜单失败: {}", e))?;

    menu_builder = menu_builder.separator().item(&quit_item);

    menu_builder
        .build()
        .map_err(|e| format!("构建菜单失败: {}", e))
}

#[cfg(target_os = "macos")]
fn apply_tray_policy(app: &tauri::AppHandle, dock_visible: bool) {
    let desired_policy = if dock_visible {
        ActivationPolicy::Regular
    } else {
        ActivationPolicy::Accessory
    };

    if let Err(err) = app.set_dock_visibility(dock_visible) {
        log::warn!("设置 Dock 显示状态失败: {}", err);
    }

    if let Err(err) = app.set_activation_policy(desired_policy) {
        log::warn!("设置激活策略失败: {}", err);
    }
}

/// 处理托盘菜单事件
fn handle_tray_menu_event(app: &tauri::AppHandle, event_id: &str) {
    log::info!("处理托盘菜单事件: {}", event_id);

    match event_id {
        "show_main" => {
            if let Some(window) = app.get_webview_window("main") {
                #[cfg(target_os = "windows")]
                {
                    let _ = window.set_skip_taskbar(false);
                }
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
                #[cfg(target_os = "macos")]
                {
                    apply_tray_policy(app, true);
                }
            }
        }
        "quit" => {
            log::info!("退出应用");
            app.exit(0);
        }
        "claude_disable" => {
            log::info!("停用Claude供应商");

            // 执行停用
            let app_handle = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) =
                    disable_provider_internal(&app_handle, crate::app_config::AppType::Claude).await
                {
                    log::error!("停用Claude供应商失败: {}", e);
                }
            });
        }
        id if id.starts_with("claude_") => {
            let provider_id = id.strip_prefix("claude_").unwrap();
            log::info!("切换到Claude供应商: {}", provider_id);

            // 执行切换
            let app_handle = app.clone();
            let provider_id = provider_id.to_string();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = switch_provider_internal(
                    &app_handle,
                    crate::app_config::AppType::Claude,
                    provider_id,
                )
                .await
                {
                    log::error!("切换Claude供应商失败: {}", e);
                }
            });
        }
        "droid_disable" => {
            log::info!("停用Droid供应商");

            // 执行停用
            let app_handle = app.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) =
                    disable_provider_internal(&app_handle, crate::app_config::AppType::Droid).await
                {
                    log::error!("停用Droid供应商失败: {}", e);
                }
            });
        }
        id if id.starts_with("codex_") => {
            let provider_id = id.strip_prefix("codex_").unwrap();
            log::info!("切换到Codex供应商: {}", provider_id);

            // 执行切换
            let app_handle = app.clone();
            let provider_id = provider_id.to_string();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = switch_provider_internal(
                    &app_handle,
                    crate::app_config::AppType::Codex,
                    provider_id,
                )
                .await
                {
                    log::error!("切换Codex供应商失败: {}", e);
                }
            });
        }
        id if id.starts_with("droid_") => {
            let provider_id = id.strip_prefix("droid_").unwrap();
            log::info!("切换到Droid供应商: {}", provider_id);

            // 执行切换
            let app_handle = app.clone();
            let provider_id = provider_id.to_string();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = switch_provider_internal(
                    &app_handle,
                    crate::app_config::AppType::Droid,
                    provider_id,
                )
                .await
                {
                    log::error!("切换Droid供应商失败: {}", e);
                }
            });
        }
        id if id.starts_with("claude_endpoint_") => {
            // 格式: claude_endpoint_{provider_id}_{encoded_url}
            let parts = id.strip_prefix("claude_endpoint_").unwrap();
            if let Some(last_underscore) = parts.rfind('_') {
                let provider_id = &parts[..last_underscore];
                let url_encoded = &parts[last_underscore + 1..];
                let url = url_encoded.replace("___", "://").replace("__", "/");
                
                log::info!("一步切换：供应商 {} → 端点 {}", provider_id, url);

                let app_handle = app.clone();
                let provider_id = provider_id.to_string();
                let url = url.to_string();
                
                tauri::async_runtime::spawn(async move {
                    if let Some(app_state) = app_handle.try_state::<AppState>() {
                        // 步骤 1: 切换到该供应商
                        if let Err(e) = switch_provider_internal(
                            &app_handle,
                            crate::app_config::AppType::Claude,
                            provider_id.clone(),
                        ).await {
                            log::error!("切换供应商失败: {}", e);
                            return;
                        }

                        // 步骤 2: 切换端点
                        match commands::switch_provider_url(app_state, url.clone()).await {
                            Ok(_) => {
                                log::info!("成功切换到端点: {}", url);
                                // 更新托盘菜单
                                if let Some(app_state) = app_handle.try_state::<AppState>() {
                                    if let Ok(new_menu) = create_tray_menu(&app_handle, app_state.inner()) {
                                        if let Some(tray) = app_handle.tray_by_id("main") {
                                            let _ = tray.set_menu(Some(new_menu));
                                        }
                                    }
                                }
                            }
                            Err(e) => log::error!("切换端点失败: {}", e),
                        }
                    }
                });
            }
        }
        id if id.starts_with("claude_endpoint_") => {
            // 格式: claude_endpoint_{provider_id}_{encoded_url}
            // 找到最后一个下划线，分离 provider_id 和 url
            let parts = id.strip_prefix("claude_endpoint_").unwrap();
            
            // 使用 rfind 找到最后一个下划线的位置
            if let Some(last_underscore) = parts.rfind("___") {
                // URL 中包含 ___ (://), 从这里分割
                let provider_id = &parts[..last_underscore];
                let url_with_protocol = &parts[last_underscore..];
                let url = url_with_protocol.replace("___", "://").replace("__", "/");
                
                log::info!("一步切换：供应商 {} → 端点 {}", provider_id, url);

                let app_handle = app.clone();
                let provider_id = provider_id.to_string();
                let url = url.to_string();
                
                tauri::async_runtime::spawn(async move {
                    // 步骤 1: 切换到该供应商
                    if let Err(e) = switch_provider_internal(
                        &app_handle,
                        crate::app_config::AppType::Claude,
                        provider_id.clone(),
                    ).await {
                        log::error!("切换供应商失败: {}", e);
                        return;
                    }

                    // 步骤 2: 切换端点
                    if let Some(app_state) = app_handle.try_state::<AppState>() {
                        match commands::switch_provider_url(app_state, url.clone()).await {
                            Ok(_) => {
                                log::info!("✅ 成功一步切换到端点: {}", url);
                                // 更新托盘菜单
                                if let Some(app_state) = app_handle.try_state::<AppState>() {
                                    if let Ok(new_menu) = create_tray_menu(&app_handle, app_state.inner()) {
                                        if let Some(tray) = app_handle.tray_by_id("main") {
                                            let _ = tray.set_menu(Some(new_menu));
                                        }
                                    }
                                }
                            }
                            Err(e) => log::error!("切换端点失败: {}", e),
                        }
                    }
                });
            }
        }
        id if id.starts_with("switch_url_") => {
            // 兼容旧格式（如果还有用到）
            let url_encoded = id.strip_prefix("switch_url_").unwrap();
            let url = url_encoded.replace("___", "://").replace("__", "/");
            log::info!("切换API地址到: {}", url);

            let app_handle = app.clone();
            let url = url.to_string();
            tauri::async_runtime::spawn(async move {
                if let Some(app_state) = app_handle.try_state::<AppState>() {
                    match commands::switch_provider_url(app_state, url.clone()).await {
                        Ok(_) => {
                            log::info!("成功切换API地址到: {}", url);
                            if let Some(app_state) = app_handle.try_state::<AppState>() {
                                if let Ok(new_menu) = create_tray_menu(&app_handle, app_state.inner()) {
                                    if let Some(tray) = app_handle.tray_by_id("main") {
                                        let _ = tray.set_menu(Some(new_menu));
                                    }
                                }
                            }
                        }
                        Err(e) => log::error!("切换API地址失败: {}", e),
                    }
                }
            });
        }
        _ => {
            log::warn!("未处理的菜单事件: {}", event_id);
        }
    }
}

//

/// 内部切换供应商函数
async fn switch_provider_internal(
    app: &tauri::AppHandle,
    app_type: crate::app_config::AppType,
    provider_id: String,
) -> Result<(), String> {
    if let Some(app_state) = app.try_state::<AppState>() {
        // 在使用前先保存需要的值
        let app_type_str = app_type.as_str().to_string();
        let provider_id_clone = provider_id.clone();

        crate::commands::switch_provider(
            app_state.clone(),
            Some(app_type),
            None,
            None,
            provider_id,
        )
        .await?;

        // 切换成功后重新创建托盘菜单
        if let Ok(new_menu) = create_tray_menu(app, app_state.inner()) {
            if let Some(tray) = app.tray_by_id("main") {
                if let Err(e) = tray.set_menu(Some(new_menu)) {
                    log::error!("更新托盘菜单失败: {}", e);
                }
            }
        }

        // 发射事件到前端，通知供应商已切换
        let event_data = serde_json::json!({
            "appType": app_type_str,
            "providerId": provider_id_clone
        });
        if let Err(e) = app.emit("provider-switched", event_data) {
            log::error!("发射供应商切换事件失败: {}", e);
        }
    }
    Ok(())
}

/// 内部停用供应商函数
async fn disable_provider_internal(
    app: &tauri::AppHandle,
    app_type: crate::app_config::AppType,
) -> Result<(), String> {
    if let Some(app_state) = app.try_state::<AppState>() {
        crate::commands::disable_current_provider(app_state.clone(), Some(app_type), None, None)
            .await?;

        // 停用成功后重新创建托盘菜单
        if let Ok(new_menu) = create_tray_menu(app, app_state.inner()) {
            if let Some(tray) = app.tray_by_id("main") {
                if let Err(e) = tray.set_menu(Some(new_menu)) {
                    log::error!("更新托盘菜单失败: {}", e);
                }
            }
        }

        // 发射事件到前端，通知供应商已停用
        let event_data = serde_json::json!({
            "appType": app_type.as_str(),
            "providerId": ""
        });
        if let Err(e) = app.emit("provider-switched", event_data) {
            log::error!("发射供应商停用事件失败: {}", e);
        }
    }
    Ok(())
}

/// 全局快捷键防抖时间戳
fn last_shortcut_trigger() -> &'static Mutex<Option<Instant>> {
    static LAST_TRIGGER: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
    LAST_TRIGGER.get_or_init(|| Mutex::new(None))
}

/// 切换主窗口显示/隐藏（带防抖机制）
pub(crate) fn toggle_main_window(app: &tauri::AppHandle) {
    // 防抖：300ms 内只响应一次
    let mut last_time = last_shortcut_trigger().lock().unwrap();
    let now = Instant::now();
    
    if let Some(last) = *last_time {
        if now.duration_since(last) < Duration::from_millis(300) {
            log::debug!("快捷键触发过快，已忽略（防抖）");
            return;
        }
    }
    
    *last_time = Some(now);
    drop(last_time);
    
    if let Some(window) = app.get_webview_window("main") {
        if let Ok(is_visible) = window.is_visible() {
            if is_visible {
                // 窗口可见，则隐藏
                let _ = window.hide();
                #[cfg(target_os = "windows")]
                {
                    let _ = window.set_skip_taskbar(true);
                }
                #[cfg(target_os = "macos")]
                {
                    apply_tray_policy(app, false);
                }
            } else {
                // 窗口隐藏，则显示
                #[cfg(target_os = "windows")]
                {
                    let _ = window.set_skip_taskbar(false);
                }
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
                #[cfg(target_os = "macos")]
                {
                    apply_tray_policy(app, true);
                }
            }
        }
    }
}

/// 更新托盘菜单的Tauri命令
#[tauri::command]
async fn update_tray_menu(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<bool, String> {
    if let Ok(new_menu) = create_tray_menu(&app, state.inner()) {
        if let Some(tray) = app.tray_by_id("main") {
            tray.set_menu(Some(new_menu))
                .map_err(|e| format!("更新托盘菜单失败: {}", e))?;
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default();

    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }));
    }

    let builder = builder
        // 拦截窗口关闭：根据设置决定是否最小化到托盘
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let settings = crate::settings::get_settings();

                if settings.minimize_to_tray_on_close {
                    api.prevent_close();
                    let _ = window.hide();
                    #[cfg(target_os = "windows")]
                    {
                        let _ = window.set_skip_taskbar(true);
                    }
                    #[cfg(target_os = "macos")]
                    {
                        apply_tray_policy(&window.app_handle(), false);
                    }
                } else {
                    window.app_handle().exit(0);
                }
            }
        })
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            // 注册 Updater 插件（桌面端）
            #[cfg(desktop)]
            {
                if let Err(e) = app
                    .handle()
                    .plugin(tauri_plugin_updater::Builder::new().build())
                {
                    // 若配置不完整（如缺少 pubkey），跳过 Updater 而不中断应用
                    log::warn!("初始化 Updater 插件失败，已跳过：{}", e);
                }
            }
            #[cfg(target_os = "macos")]
            {
                // 设置 macOS 标题栏背景色为主界面蓝色
                if let Some(window) = app.get_webview_window("main") {
                    use objc2::rc::Retained;
                    use objc2::runtime::AnyObject;
                    use objc2_app_kit::NSColor;

                    let ns_window_ptr = window.ns_window().unwrap();
                    let ns_window: Retained<AnyObject> =
                        unsafe { Retained::retain(ns_window_ptr as *mut AnyObject).unwrap() };

                    // 使用与主界面 banner 相同的蓝色 #3498db
                    // #3498db = RGB(52, 152, 219)
                    let bg_color = unsafe {
                        NSColor::colorWithRed_green_blue_alpha(
                            52.0 / 255.0,  // R: 52
                            152.0 / 255.0, // G: 152
                            219.0 / 255.0, // B: 219
                            1.0,           // Alpha: 1.0
                        )
                    };

                    unsafe {
                        use objc2::msg_send;
                        let _: () = msg_send![&*ns_window, setBackgroundColor: &*bg_color];
                    }
                }
            }

            // 初始化日志
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            // 初始化应用状态（仅创建一次，并在本函数末尾注入 manage）
            let app_state = AppState::new();

            // 首次启动迁移：扫描副本文件，合并到 config.json，并归档副本；旧 config.json 先归档
            {
                let mut config_guard = app_state.config.lock().unwrap();
                let migrated = migration::migrate_copies_into_config(&mut config_guard)?;
                if migrated {
                    log::info!("已将副本文件导入到 config.json，并完成归档");
                }
                // 确保两个 App 条目存在
                config_guard.ensure_app(&app_config::AppType::Claude);
                config_guard.ensure_app(&app_config::AppType::Codex);
            }

            // 保存配置
            let _ = app_state.save();

            // 创建动态托盘菜单
            let menu = create_tray_menu(app.handle(), &app_state)?;

            // 构建托盘 - 使用防抖机制避免双击时触发两次切换
            let app_handle_for_tray = app.handle().clone();
            let last_click_time = Arc::new(Mutex::new(None::<Instant>));
            let last_click_time_clone = last_click_time.clone();

            let mut tray_builder = TrayIconBuilder::with_id("main")
                .on_tray_icon_event(move |_tray, event| match event {
                    // 左键单击切换窗口显示/隐藏（带防抖）
                    TrayIconEvent::Click { button, .. } => {
                        if matches!(button, tauri::tray::MouseButton::Left) {
                            let now = Instant::now();
                            let mut last_time = last_click_time_clone.lock().unwrap();

                            // 防抖：如果距离上次点击少于 300ms，忽略（这是双击的第二次点击）
                            if let Some(last) = *last_time {
                                if now.duration_since(last).as_millis() < 300 {
                                    return;
                                }
                            }

                            // 更新上次点击时间
                            *last_time = Some(now);
                            drop(last_time);

                            if let Some(window) = app_handle_for_tray.get_webview_window("main") {
                                // 检查窗口是否可见
                                if let Ok(is_visible) = window.is_visible() {
                                    if is_visible {
                                        // 窗口可见，则隐藏
                                        let _ = window.hide();
                                        #[cfg(target_os = "windows")]
                                        {
                                            let _ = window.set_skip_taskbar(true);
                                        }
                                        #[cfg(target_os = "macos")]
                                        {
                                            apply_tray_policy(&app_handle_for_tray, false);
                                        }
                                    } else {
                                        // 窗口隐藏，则显示
                                        #[cfg(target_os = "windows")]
                                        {
                                            let _ = window.set_skip_taskbar(false);
                                        }
                                        let _ = window.unminimize();
                                        let _ = window.show();
                                        let _ = window.set_focus();
                                        #[cfg(target_os = "macos")]
                                        {
                                            apply_tray_policy(&app_handle_for_tray, true);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    _ => log::debug!("unhandled event {event:?}"),
                })
                .menu(&menu)
                .on_menu_event(|app, event| {
                    handle_tray_menu_event(app, &event.id.0);
                })
                .show_menu_on_left_click(false);

            // 统一使用应用默认图标；待托盘模板图标就绪后再启用
            tray_builder = tray_builder.icon(app.default_window_icon().unwrap().clone());

            let _tray = tray_builder.build(app)?;
            // 将同一个实例注入到全局状态，避免重复创建导致的不一致
            app.manage(app_state);

            // 注册全局快捷键
            let settings = crate::settings::get_settings();
            if let Some(shortcut_str) = settings.global_shortcut {
                if !shortcut_str.is_empty() {
                    match Shortcut::from_str(&shortcut_str) {
                        Ok(shortcut) => {
                            let app_handle = app.handle().clone();
                            match app.global_shortcut().on_shortcut(shortcut, move |_app, _shortcut, _event| {
                                // 使用防抖机制避免快速重复触发
                                toggle_main_window(&app_handle);
                            }) {
                                Ok(_) => {
                                    log::info!("已注册全局快捷键: {}", shortcut_str);
                                }
                                Err(e) => {
                                    log::error!("注册全局快捷键失败: {}", e);
                                    // 通知前端显示错误
                                    let error_msg = format!("全局快捷键 \"{}\" 注册失败: {}", shortcut_str, e);
                                    let _ = app.emit("global-shortcut-error", error_msg);
                                }
                            }
                        }
                        Err(e) => {
                            log::error!("快捷键格式无效: {} - {}", shortcut_str, e);
                            let error_msg = format!("快捷键格式无效: \"{}\" - {}", shortcut_str, e);
                            let _ = app.emit("global-shortcut-error", error_msg);
                        }
                    }
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_providers,
            commands::get_current_provider,
            commands::add_provider,
            commands::update_provider,
            commands::delete_provider,
            commands::switch_provider,
            commands::disable_current_provider,
            commands::switch_provider_url,
            commands::import_default_config,
            commands::get_claude_config_status,
            commands::get_config_status,
            commands::get_claude_code_config_path,
            commands::get_current_claude_settings,
            commands::sync_current_provider_config,
            commands::get_config_dir,
            commands::open_config_folder,
            commands::pick_directory,
            commands::open_external,
            commands::get_app_config_path,
            commands::open_app_config_folder,
            commands::get_mcp_config,
            commands::upsert_mcp_server_in_config,
            commands::delete_mcp_server_in_config,
            commands::set_mcp_enabled,
            commands::import_mcp_from_claude,
            commands::import_mcp_from_codex,
            commands::import_mcp_from_droid,
            commands::sync_enabled_mcp_to_claude,
            commands::sync_enabled_mcp_to_codex,
            commands::sync_enabled_mcp_to_droid,
            commands::check_mcp_sync_conflict,
            commands::sync_mcp_to_other_app,
            commands::get_settings,
            commands::save_settings,
            commands::get_autostart_status,
            commands::set_autostart,
            commands::register_global_shortcut,
            commands::unregister_global_shortcut,
            commands::validate_global_shortcut,
            commands::check_for_updates,
            commands::is_portable_mode,
            commands::get_vscode_settings_status,
            commands::read_vscode_settings,
            commands::write_vscode_settings,
            commands::get_claude_plugin_status,
            commands::read_claude_plugin_config,
            commands::apply_claude_plugin_config,
            commands::is_claude_plugin_applied,
            commands::test_endpoints,
            commands::check_droid_balance,
            commands::batch_check_droid_balances,
            commands::get_factory_api_key_env,
            update_tray_menu,
        ]);

    let app = builder
        .build(tauri::generate_context!())
        .expect("error while running tauri application");

    app.run(|app_handle, event| {
        #[cfg(target_os = "macos")]
        // macOS 在 Dock 图标被点击并重新激活应用时会触发 Reopen 事件，这里手动恢复主窗口
        match event {
            RunEvent::Reopen { .. } => {
                if let Some(window) = app_handle.get_webview_window("main") {
                    #[cfg(target_os = "windows")]
                    {
                        let _ = window.set_skip_taskbar(false);
                    }
                    let _ = window.unminimize();
                    let _ = window.show();
                    let _ = window.set_focus();
                    apply_tray_policy(app_handle, true);
                }
            }
            _ => {}
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = (app_handle, event);
        }
    });
}
