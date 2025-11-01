export type ProviderCategory =
  | "official" // 官方
  | "cn_official" // 国产官方
  | "aggregator" // 聚合网站
  | "third_party" // 第三方供应商
  | "custom"; // 自定义

export interface Provider {
  id: string;
  name: string;
  settingsConfig: Record<string, any>; // 应用配置对象：Claude 为 settings.json；Codex 为 { auth, config }
  websiteUrl?: string;
  // 新增：供应商分类（用于差异化提示/能力开关）
  category?: ProviderCategory;
  createdAt?: number; // 添加时间戳（毫秒）
  // 备选 URL 列表（用于快速切换 API 地址）
  alternativeUrls?: string[];
  // 可选：供应商元数据（仅存于 ~/.cc-switch/config.json，不写入 live 配置）
  meta?: ProviderMeta;
}

export interface AppConfig {
  providers: Record<string, Provider>;
  current: string;
}

// 自定义端点配置
export interface CustomEndpoint {
  url: string;
  addedAt: number;
  lastUsed?: number;
}

// 供应商元数据（字段名与后端一致，保持 snake_case）
export interface ProviderMeta {
  // 自定义端点：以 URL 为键，值为端点信息
  custom_endpoints?: Record<string, CustomEndpoint>;
}

// 应用设置类型（用于 SettingsModal 与 Tauri API）
export interface Settings {
  // 是否在系统托盘（macOS 菜单栏）显示图标
  showInTray: boolean;
  // 点击关闭按钮时是否最小化到托盘而不是关闭应用
  minimizeToTrayOnClose: boolean;
  // 覆盖 Claude Code 配置目录（可选）
  claudeConfigDir?: string;
  // 覆盖 Codex 配置目录（可选）
  codexConfigDir?: string;
  // 覆盖 Droid 配置目录（可选）
  droidConfigDir?: string;
  // 首选语言（可选，默认中文）
  language?: "en" | "zh";
  // 开机自启动
  autoStart: boolean;
  // 全局快捷键
  globalShortcut?: string;
  // Droid 余额自动刷新
  autoRefreshBalance: boolean;
  // 刷新间隔（秒）
  refreshInterval: number;
  // 仅在窗口可见时刷新
  refreshOnlyWhenVisible: boolean;
}

// MCP 服务器连接参数（宽松：允许扩展字段）
export interface McpServerSpec {
  // 可选：社区常见 .mcp.json 中 stdio 配置可不写 type
  type?: "stdio" | "http";
  // stdio 字段
  command?: string;
  args?: string[];
  env?: Record<string, string>;
  cwd?: string;
  // http 字段
  url?: string;
  headers?: Record<string, string>;
  // 通用字段
  [key: string]: any;
}

// MCP 服务器条目（含元信息）
export interface McpServer {
  id: string;
  name?: string;
  description?: string;
  tags?: string[];
  homepage?: string;
  docs?: string;
  enabled?: boolean;
  server: McpServerSpec;
  source?: string;
  [key: string]: any;
}

// MCP 配置状态
export interface McpStatus {
  userConfigPath: string;
  userConfigExists: boolean;
  serverCount: number;
}

// 新：来自 config.json 的 MCP 列表响应
export interface McpConfigResponse {
  configPath: string;
  servers: Record<string, McpServer>;
}
