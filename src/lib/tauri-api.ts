import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import { Provider, Settings, McpConfigResponse, McpServer } from "../types";

// 应用类型
export type AppType = "claude" | "codex" | "droid";

// 定义配置状态类型
interface ConfigStatus {
  exists: boolean;
  path: string;
  error?: string;
}

// 定义导入结果类型
interface ImportResult {
  success: boolean;
  message?: string;
}

// 定义端点延迟测试结果类型
export interface EndpointLatency {
  url: string;
  latency: number | null;
  status: number | null;
  error: string | null;
}

// Droid 余额信息类型
export interface BalanceInfo {
  used: number;
  allowance: number;
  remaining: number;
  overage: number;
  usedRatio: number;
  percentUsed: number;
  exceeded: boolean;
  expiryDate?: string;
}

// Tauri API 封装，提供统一的全局 API 接口
export const tauriAPI = {
  // 获取所有供应商
  getProviders: async (app?: AppType): Promise<Record<string, Provider>> => {
    try {
      return await invoke("get_providers", { app_type: app, app });
    } catch (error) {
      console.error("获取供应商列表失败:", error);
      return {};
    }
  },

  // 获取当前供应商ID
  getCurrentProvider: async (app?: AppType): Promise<string> => {
    try {
      return await invoke("get_current_provider", { app_type: app, app });
    } catch (error) {
      console.error("获取当前供应商失败:", error);
      return "";
    }
  },

  // 添加供应商
  addProvider: async (provider: Provider, app?: AppType): Promise<boolean> => {
    try {
      return await invoke("add_provider", { provider, app_type: app, app });
    } catch (error) {
      console.error("添加供应商失败:", error);
      throw error;
    }
  },

  // 更新供应商
  updateProvider: async (
    provider: Provider,
    app?: AppType,
  ): Promise<boolean> => {
    try {
      return await invoke("update_provider", { provider, app_type: app, app });
    } catch (error) {
      console.error("更新供应商失败:", error);
      throw error;
    }
  },

  // 删除供应商
  deleteProvider: async (id: string, app?: AppType): Promise<boolean> => {
    try {
      return await invoke("delete_provider", { id, app_type: app, app });
    } catch (error) {
      console.error("删除供应商失败:", error);
      throw error;
    }
  },

  // 切换供应商
  switchProvider: async (
    providerId: string,
    app?: AppType,
  ): Promise<boolean> => {
    try {
      return await invoke("switch_provider", {
        id: providerId,
        app_type: app,
        app,
      });
    } catch (error) {
      console.error("切换供应商失败:", error);
      return false;
    }
  },

  // 停用当前供应商
  disableCurrentProvider: async (app?: AppType): Promise<boolean> => {
    try {
      return await invoke("disable_current_provider", { app_type: app, app });
    } catch (error) {
      console.error("停用当前供应商失败:", error);
      throw error;
    }
  },

  // 导入当前配置为默认供应商
  importCurrentConfigAsDefault: async (
    app?: AppType,
  ): Promise<ImportResult> => {
    try {
      const success = await invoke<boolean>("import_default_config", {
        app_type: app,
        app,
      });
      return {
        success,
        message: success ? "成功导入默认配置" : "导入失败",
      };
    } catch (error) {
      console.error("导入默认配置失败:", error);
      return {
        success: false,
        message: String(error),
      };
    }
  },

  // 获取 Claude Code 配置文件路径
  getClaudeCodeConfigPath: async (): Promise<string> => {
    try {
      return await invoke("get_claude_code_config_path");
    } catch (error) {
      console.error("获取配置路径失败:", error);
      return "";
    }
  },

  // 获取 Claude Code 配置状态
  getClaudeConfigStatus: async (): Promise<ConfigStatus> => {
    try {
      return await invoke("get_claude_config_status");
    } catch (error) {
      console.error("获取配置状态失败:", error);
      return {
        exists: false,
        path: "",
        error: String(error),
      };
    }
  },

  // 获取应用配置状态（通用）
  getConfigStatus: async (app?: AppType): Promise<ConfigStatus> => {
    try {
      return await invoke("get_config_status", { app_type: app, app });
    } catch (error) {
      console.error("获取配置状态失败:", error);
      return {
        exists: false,
        path: "",
        error: String(error),
      };
    }
  },

  // 打开配置文件夹
  openConfigFolder: async (app?: AppType): Promise<void> => {
    try {
      await invoke("open_config_folder", { app_type: app, app });
    } catch (error) {
      console.error("打开配置文件夹失败:", error);
    }
  },

  // 打开外部链接
  openExternal: async (url: string): Promise<void> => {
    try {
      await invoke("open_external", { url });
    } catch (error) {
      console.error("打开外部链接失败:", error);
    }
  },

  // 更新托盘菜单
  updateTrayMenu: async (): Promise<boolean> => {
    try {
      return await invoke("update_tray_menu");
    } catch (error) {
      console.error("更新托盘菜单失败:", error);
      return false;
    }
  },

  // 监听供应商切换事件
  onProviderSwitched: async (
    callback: (data: { appType: string; providerId: string }) => void,
  ): Promise<UnlistenFn> => {
    return await listen("provider-switched", (event) => {
      callback(event.payload as { appType: string; providerId: string });
    });
  },

  // （保留空位，取消迁移提示）

  // 选择配置文件（Tauri 暂不实现，保留接口兼容性）
  selectConfigFile: async (): Promise<string | null> => {
    console.warn("selectConfigFile 在 Tauri 版本中暂不支持");
    return null;
  },

  // 获取设置
  getSettings: async (): Promise<Settings> => {
    try {
      return await invoke("get_settings");
    } catch (error) {
      console.error("获取设置失败:", error);
      return {
        showInTray: true,
        minimizeToTrayOnClose: false,
        language: "zh",
        autoStart: false,
        globalShortcut: undefined,
      };
    }
  },

  // 保存设置
  saveSettings: async (settings: Settings): Promise<boolean> => {
    try {
      return await invoke("save_settings", { settings });
    } catch (error) {
      console.error("保存设置失败:", error);
      return false;
    }
  },

  // 获取开机自启动状态
  getAutostartStatus: async (): Promise<boolean> => {
    try {
      return await invoke("get_autostart_status");
    } catch (error) {
      console.error("获取开机自启动状态失败:", error);
      return false;
    }
  },

  // 设置开机自启动
  setAutostart: async (enable: boolean): Promise<boolean> => {
    try {
      return await invoke("set_autostart", { enable });
    } catch (error) {
      console.error("设置开机自启动失败:", error);
      return false;
    }
  },

  // 注册全局快捷键
  registerGlobalShortcut: async (shortcut: string): Promise<boolean> => {
    try {
      return await invoke("register_global_shortcut", { shortcut });
    } catch (error) {
      console.error("注册全局快捷键失败:", error);
      return false;
    }
  },

  // 注销全局快捷键
  unregisterGlobalShortcut: async (shortcut: string): Promise<boolean> => {
    try {
      return await invoke("unregister_global_shortcut", { shortcut });
    } catch (error) {
      console.error("注销全局快捷键失败:", error);
      return false;
    }
  },

  // 验证全局快捷键
  validateGlobalShortcut: async (shortcut: string): Promise<boolean> => {
    try {
      return await invoke("validate_global_shortcut", { shortcut });
    } catch (error) {
      console.error("验证全局快捷键失败:", error);
      return false;
    }
  },

  // 检查更新
  checkForUpdates: async (): Promise<void> => {
    try {
      await invoke("check_for_updates");
    } catch (error) {
      console.error("检查更新失败:", error);
    }
  },

  // 获取应用配置文件路径
  getAppConfigPath: async (): Promise<string> => {
    try {
      return await invoke("get_app_config_path");
    } catch (error) {
      console.error("获取应用配置路径失败:", error);
      return "";
    }
  },

  // 打开应用配置文件夹
  openAppConfigFolder: async (): Promise<void> => {
    try {
      await invoke("open_app_config_folder");
    } catch (error) {
      console.error("打开应用配置文件夹失败:", error);
    }
  },

  // 选择配置目录
  selectConfigDirectory: async (
    defaultPath?: string,
  ): Promise<string | null> => {
    try {
      return await invoke("pick_directory", {
        default_path: defaultPath,
      });
    } catch (error) {
      console.error("选择配置目录失败:", error);
      return null;
    }
  },

  // 获取配置目录
  getConfigDir: async (app?: AppType): Promise<string> => {
    try {
      return await invoke("get_config_dir", { app_type: app, app });
    } catch (error) {
      console.error("获取配置目录失败:", error);
      return "";
    }
  },

  // 判断是否为便携版
  isPortable: async (): Promise<boolean> => {
    try {
      return await invoke("is_portable_mode");
    } catch (error) {
      console.error("判断便携版失败:", error);
      return false;
    }
  },

  // VS Code: 获取 settings.json 状态
  getVSCodeSettingsStatus: async (): Promise<{
    exists: boolean;
    path: string;
    error?: string;
  }> => {
    try {
      return await invoke("get_vscode_settings_status");
    } catch (error) {
      console.error("获取 VS Code 设置状态失败:", error);
      return { exists: false, path: "", error: String(error) };
    }
  },

  // VS Code: 读取 settings.json 文本
  readVSCodeSettings: async (): Promise<string> => {
    try {
      return await invoke("read_vscode_settings");
    } catch (error) {
      throw new Error(`读取 VS Code 设置失败: ${String(error)}`);
    }
  },

  // VS Code: 写回 settings.json 文本（不自动创建）
  writeVSCodeSettings: async (content: string): Promise<boolean> => {
    try {
      return await invoke("write_vscode_settings", { content });
    } catch (error) {
      throw new Error(`写入 VS Code 设置失败: ${String(error)}`);
    }
  },

  // 获取当前 Claude settings.json 的完整内容
  getCurrentClaudeSettings: async (): Promise<any> => {
    try {
      return await invoke("get_current_claude_settings");
    } catch (error) {
      throw new Error(`获取 Claude 配置失败: ${String(error)}`);
    }
  },

  // 同步当前供应商配置（从 live settings.json 回填）
  syncCurrentProviderConfig: async (app?: AppType): Promise<boolean> => {
    try {
      return await invoke("sync_current_provider_config", {
        app_type: app,
        app,
      });
    } catch (error) {
      throw new Error(`同步供应商配置失败: ${String(error)}`);
    }
  },

  // ========== MCP 相关 API ==========

  // 获取 MCP 配置
  getMcpConfig: async (app?: AppType): Promise<McpConfigResponse> => {
    try {
      return await invoke("get_mcp_config", { app_type: app, app });
    } catch (error) {
      console.error("获取 MCP 配置失败:", error);
      throw error;
    }
  },

  // 添加或更新 MCP 服务器
  upsertMcpServerInConfig: async (
    app: AppType,
    id: string,
    server: McpServer,
  ): Promise<boolean> => {
    try {
      return await invoke("upsert_mcp_server_in_config", {
        app_type: app,
        app,
        id,
        spec: server,
      });
    } catch (error) {
      console.error("添加/更新 MCP 服务器失败:", error);
      throw error;
    }
  },

  // 删除 MCP 服务器
  deleteMcpServerInConfig: async (
    app: AppType,
    id: string,
  ): Promise<boolean> => {
    try {
      return await invoke("delete_mcp_server_in_config", {
        app_type: app,
        app,
        id,
      });
    } catch (error) {
      console.error("删除 MCP 服务器失败:", error);
      throw error;
    }
  },

  // 设置 MCP 服务器启用状态
  setMcpEnabled: async (
    app: AppType,
    id: string,
    enabled: boolean,
  ): Promise<boolean> => {
    try {
      return await invoke("set_mcp_enabled", {
        app_type: app,
        app,
        id,
        enabled,
      });
    } catch (error) {
      console.error("设置 MCP 启用状态失败:", error);
      throw error;
    }
  },

  // 从 Claude 导入 MCP 配置
  importMcpFromClaude: async (): Promise<boolean> => {
    try {
      return await invoke("import_mcp_from_claude");
    } catch (error) {
      console.error("从 Claude 导入 MCP 失败:", error);
      throw error;
    }
  },

  // 从 Codex 导入 MCP 配置
  importMcpFromCodex: async (): Promise<boolean> => {
    try {
      return await invoke("import_mcp_from_codex");
    } catch (error) {
      console.error("从 Codex 导入 MCP 失败:", error);
      throw error;
    }
  },

  // 从 Droid 导入 MCP 配置
  importMcpFromDroid: async (): Promise<boolean> => {
    try {
      return await invoke("import_mcp_from_droid");
    } catch (error) {
      console.error("从 Droid 导入 MCP 失败:", error);
      throw error;
    }
  },

  // 同步已启用的 MCP 到 Claude
  syncEnabledMcpToClaude: async (): Promise<boolean> => {
    try {
      return await invoke("sync_enabled_mcp_to_claude");
    } catch (error) {
      console.error("同步 MCP 到 Claude 失败:", error);
      throw error;
    }
  },

  // 同步已启用的 MCP 到 Codex
  syncEnabledMcpToCodex: async (): Promise<boolean> => {
    try {
      return await invoke("sync_enabled_mcp_to_codex");
    } catch (error) {
      console.error("同步 MCP 到 Codex 失败:", error);
      throw error;
    }
  },

  // 同步已启用的 MCP 到 Droid
  syncEnabledMcpToDroid: async (): Promise<boolean> => {
    try {
      return await invoke("sync_enabled_mcp_to_droid");
    } catch (error) {
      console.error("同步 MCP 到 Droid 失败:", error);
      throw error;
    }
  },

  // v3.5.1: 检查目标应用中是否存在同名 MCP 服务器
  checkMcpSyncConflict: async (targetApp: AppType, id: string): Promise<boolean> => {
    try {
      return await invoke("check_mcp_sync_conflict", { targetApp, id });
    } catch (error) {
      console.error("检查 MCP 同步冲突失败:", error);
      throw error;
    }
  },

  // v3.5.1+: 同步 MCP 到目标应用（三向同步：Claude ↔ Codex ↔ Droid）
  syncMcpToOtherApp: async (
    sourceApp: AppType,
    targetApp: AppType,
    id: string,
    overwrite: boolean,
  ): Promise<boolean> => {
    try {
      return await invoke("sync_mcp_to_other_app", { sourceApp, targetApp, id, overwrite });
    } catch (error) {
      console.error("同步 MCP 到目标应用失败:", error);
      throw error;
    }
  },

  // ========== 端点测速 API ==========

  // 测试端点速度
  testEndpoints: async (
    urls: string[],
    timeoutSecs?: number,
  ): Promise<EndpointLatency[]> => {
    try {
      return await invoke("test_endpoints", {
        urls,
        timeout_secs: timeoutSecs,
      });
    } catch (error) {
      console.error("测试端点速度失败:", error);
      throw error;
    }
  },

  // ========== Droid 余额查询 API ==========

  // 查询单个密钥余额
  checkDroidBalance: async (apiKey: string): Promise<BalanceInfo> => {
    try {
      console.log("[API] 调用 check_droid_balance，API Key:", apiKey?.substring(0, 10) + "...");
      const result = await invoke("check_droid_balance", { apiKey: apiKey });
      console.log("[API] check_droid_balance 返回:", result);
      return result;
    } catch (error) {
      console.error("[API] 查询 Droid 余额失败:", error);
      throw error;
    }
  },

  // 批量查询余额
  batchCheckDroidBalances: async (
    apiKeys: string[],
  ): Promise<Record<string, BalanceInfo>> => {
    try {
      return await invoke("batch_check_droid_balances", { apiKeys: apiKeys });
    } catch (error) {
      console.error("批量查询 Droid 余额失败:", error);
      throw error;
    }
  },

  // 获取当前环境变量值
  getFactoryApiKeyEnv: async (): Promise<string | null> => {
    try {
      return await invoke("get_factory_api_key_env");
    } catch (error) {
      console.error("获取环境变量失败:", error);
      return null;
    }
  },
};

// 创建全局 API 对象，兼容现有代码
if (typeof window !== "undefined") {
  // 绑定到 window.api，避免 Electron 命名造成误解
  // API 内部已做 try/catch，非 Tauri 环境下也会安全返回默认值
  (window as any).api = tauriAPI;
}

export default tauriAPI;
