import React, { useEffect, useState, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { Provider } from "../types";
import { Play, Edit3, Trash2, CheckCircle2, Users, Check, Zap, Loader2 } from "lucide-react";
import { buttonStyles, cardStyles, badgeStyles, cn } from "../lib/styles";
import { AppType, EndpointLatency, BalanceInfo, tauriAPI } from "../lib/tauri-api";
import { BalanceDisplay } from "./BalanceDisplay";
import {
  applyProviderToVSCode,
  detectApplied,
  normalizeBaseUrl,
} from "../utils/vscodeSettings";
import { getCodexBaseUrl } from "../utils/providerConfigUtils";
import { useVSCodeAutoSync } from "../hooks/useVSCodeAutoSync";
import { useAutoRefresh } from "../hooks/useAutoRefresh";
// 不再在列表中显示分类徽章，避免造成困惑

interface ProviderListProps {
  providers: Record<string, Provider>;
  currentProviderId: string;
  onSwitch: (id: string) => void;
  onDisable: () => void;
  onDelete: (id: string) => void;
  onEdit: (id: string) => void;
  appType?: AppType;
  onNotify?: (
    message: string,
    type: "success" | "error",
    duration?: number,
  ) => void;
}

const ProviderList: React.FC<ProviderListProps> = ({
  providers,
  currentProviderId,
  onSwitch,
  onDisable,
  onDelete,
  onEdit,
  appType,
  onNotify,
}) => {
  const { t } = useTranslation();
  // 提取API地址（兼容不同供应商配置：Claude env / Codex TOML）
  const getApiUrl = (provider: Provider): string => {
    try {
      const cfg = provider.settingsConfig;
      // Claude/Anthropic: 从 env 中读取
      if (cfg?.env?.ANTHROPIC_BASE_URL) {
        return cfg.env.ANTHROPIC_BASE_URL;
      }
      // Codex: 从 TOML 配置中解析 base_url
      if (typeof cfg?.config === "string" && cfg.config.includes("base_url")) {
        // 支持单/双引号
        const match = cfg.config.match(/base_url\s*=\s*(['"])([^'\"]+)\1/);
        if (match && match[2]) return match[2];
      }
      return t("provider.notConfigured");
    } catch {
      return t("provider.configError");
    }
  };

  const handleUrlClick = async (url: string) => {
    try {
      await window.api.openExternal(url);
    } catch (error) {
      console.error(t("console.openLinkFailed"), error);
    }
  };

  // 解析 Codex 配置中的 base_url（已提取到公共工具）

  // VS Code 按钮：仅在 Codex + 当前供应商显示；按钮文案根据是否"已应用"变化
  const [vscodeAppliedFor, setVscodeAppliedFor] = useState<string | null>(null);
  const { enableAutoSync, disableAutoSync } = useVSCodeAutoSync();
  const [claudeApplied, setClaudeApplied] = useState<boolean>(false);

  // 当当前供应商或 appType 变化时，尝试读取 VS Code settings 并检测状态
  useEffect(() => {
    const check = async () => {
      if (appType !== "codex" || !currentProviderId) {
        setVscodeAppliedFor(null);
        return;
      }
      const status = await window.api.getVSCodeSettingsStatus();
      if (!status.exists) {
        setVscodeAppliedFor(null);
        return;
      }
      try {
        const content = await window.api.readVSCodeSettings();
        const detected = detectApplied(content);
        // 认为“已应用”的条件（非官方供应商）：VS Code 中的 apiBase 与当前供应商的 base_url 完全一致
        const current = providers[currentProviderId];
        let applied = false;
        if (current && current.category !== "official") {
          const base = getCodexBaseUrl(current);
          if (detected.apiBase && base) {
            applied =
              normalizeBaseUrl(detected.apiBase) === normalizeBaseUrl(base);
          }
        }
        setVscodeAppliedFor(applied ? currentProviderId : null);
      } catch {
        setVscodeAppliedFor(null);
      }
    };
    check();
  }, [appType, currentProviderId, providers]);

  // 检查 Claude 插件配置是否已应用
  useEffect(() => {
    const checkClaude = async () => {
      if (appType !== "claude" || !currentProviderId) {
        setClaudeApplied(false);
        return;
      }
      try {
        const applied = await window.api.isClaudePluginApplied();
        setClaudeApplied(applied);
      } catch (error) {
        console.error("检测 Claude 插件配置失败:", error);
        setClaudeApplied(false);
      }
    };
    checkClaude();
  }, [appType, currentProviderId, providers]);

  const handleApplyToVSCode = async (provider: Provider) => {
    try {
      const status = await window.api.getVSCodeSettingsStatus();
      if (!status.exists) {
        onNotify?.(t("notifications.vscodeSettingsNotFound"), "error", 3000);
        return;
      }

      const raw = await window.api.readVSCodeSettings();

      const isOfficial = provider.category === "official";
      // 非官方且缺少 base_url 时直接报错并返回，避免“空写入”假成功
      if (!isOfficial) {
        const parsed = getCodexBaseUrl(provider);
        if (!parsed) {
          onNotify?.(t("notifications.missingBaseUrl"), "error", 4000);
          return;
        }
      }

      const baseUrl = isOfficial ? undefined : getCodexBaseUrl(provider);
      const next = applyProviderToVSCode(raw, { baseUrl, isOfficial });

      if (next === raw) {
        // 幂等：没有变化也提示成功
        onNotify?.(t("notifications.appliedToVSCode"), "success", 3000);
        setVscodeAppliedFor(provider.id);
        // 用户手动应用时，启用自动同步
        enableAutoSync();
        return;
      }

      await window.api.writeVSCodeSettings(next);
      onNotify?.(t("notifications.appliedToVSCode"), "success", 3000);
      setVscodeAppliedFor(provider.id);
      // 用户手动应用时，启用自动同步
      enableAutoSync();
    } catch (e: any) {
      console.error(e);
      const msg =
        e && e.message ? e.message : t("notifications.syncVSCodeFailed");
      onNotify?.(msg, "error", 5000);
    }
  };

  const handleRemoveFromVSCode = async () => {
    try {
      const status = await window.api.getVSCodeSettingsStatus();
      if (!status.exists) {
        onNotify?.(t("notifications.vscodeSettingsNotFound"), "error", 3000);
        return;
      }
      const raw = await window.api.readVSCodeSettings();
      const next = applyProviderToVSCode(raw, {
        baseUrl: undefined,
        isOfficial: true,
      });
      if (next === raw) {
        onNotify?.(t("notifications.removedFromVSCode"), "success", 3000);
        setVscodeAppliedFor(null);
        // 用户手动移除时，禁用自动同步
        disableAutoSync();
        return;
      }
      await window.api.writeVSCodeSettings(next);
      onNotify?.(t("notifications.removedFromVSCode"), "success", 3000);
      setVscodeAppliedFor(null);
      // 用户手动移除时，禁用自动同步
      disableAutoSync();
    } catch (e: any) {
      console.error(e);
      const msg =
        e && e.message ? e.message : t("notifications.syncVSCodeFailed");
      onNotify?.(msg, "error", 5000);
    }
  };

  const handleApplyToClaudePlugin = async () => {
    try {
      await window.api.applyClaudePluginConfig({ official: false });
      onNotify?.(t("notifications.appliedToClaudePlugin"), "success", 3000);
      setClaudeApplied(true);
    } catch (error: any) {
      console.error(error);
      const msg =
        error && error.message
          ? error.message
          : t("notifications.syncClaudePluginFailed");
      onNotify?.(msg, "error", 5000);
    }
  };

  const handleRemoveFromClaudePlugin = async () => {
    try {
      await window.api.applyClaudePluginConfig({ official: true });
      onNotify?.(t("notifications.removedFromClaudePlugin"), "success", 3000);
      setClaudeApplied(false);
    } catch (error: any) {
      console.error(error);
      const msg =
        error && error.message
          ? error.message
          : t("notifications.syncClaudePluginFailed");
      onNotify?.(msg, "error", 5000);
    }
  };

  // 测速状态管理
  const [testingProviderId, setTestingProviderId] = useState<string | null>(null);
  const [speedTestResults, setSpeedTestResults] = useState<Record<string, EndpointLatency>>({});

  // Droid 余额相关状态
  const [balances, setBalances] = useState<Record<string, BalanceInfo>>({});
  const [balanceLastChecked, setBalanceLastChecked] = useState<Record<string, number>>({});
  const [checkingBalance, setCheckingBalance] = useState<string | null>(null);
  const [balanceErrors, setBalanceErrors] = useState<Record<string, string>>({});

  // 自动刷新相关状态
  const [autoRefreshEnabled, setAutoRefreshEnabled] = useState(false);
  const [refreshInterval, setRefreshInterval] = useState(300);
  const [refreshOnlyWhenVisible, setRefreshOnlyWhenVisible] = useState(true);

  // 加载自动刷新设置
  useEffect(() => {
    const loadSettings = async () => {
      try {
        const settings = await tauriAPI.getSettings();
        const newEnabled = settings.autoRefreshBalance || false;
        const newInterval = settings.refreshInterval || 300;
        const newVisible = settings.refreshOnlyWhenVisible !== false;

        setAutoRefreshEnabled(newEnabled);
        setRefreshInterval(newInterval);
        setRefreshOnlyWhenVisible(newVisible);

        console.log('[ProviderList] 设置已加载:', {
          autoRefreshEnabled: newEnabled,
          refreshInterval: newInterval,
          refreshOnlyWhenVisible: newVisible,
        });
      } catch (error) {
        console.error('[ProviderList] 加载设置失败:', error);
      }
    };

    // 初始加载
    loadSettings();

    // 监听设置更新事件（设置保存后立即生效）
    const handleSettingsUpdated = (event: Event) => {
      console.log('[ProviderList] 收到 settings-updated 事件，立即重新加载设置');
      const customEvent = event as CustomEvent;
      if (customEvent.detail) {
        // 直接使用事件中的设置数据，更快
        const settings = customEvent.detail;
        setAutoRefreshEnabled(settings.autoRefreshBalance || false);
        setRefreshInterval(settings.refreshInterval || 300);
        setRefreshOnlyWhenVisible(settings.refreshOnlyWhenVisible !== false);
        console.log('[ProviderList] 设置已更新（来自事件）:', {
          autoRefreshEnabled: settings.autoRefreshBalance || false,
          refreshInterval: settings.refreshInterval || 300,
        });
      } else {
        // 如果事件中没有数据，从后端重新加载
        loadSettings();
      }
    };

    window.addEventListener('settings-updated', handleSettingsUpdated);
    return () => {
      window.removeEventListener('settings-updated', handleSettingsUpdated);
    };
  }, []);

  // 处理测速
  const handleTestSpeed = async (provider: Provider) => {
    const url = getApiUrl(provider);

    // 跳过未配置或错误的URL
    if (url === t("provider.notConfigured") || url === t("provider.configError")) {
      onNotify?.(t("provider.speedTestFailed"), "error", 3000);
      return;
    }

    setTestingProviderId(provider.id);

    try {
      const results = await window.api.testEndpoints([url], 8);
      if (results && results.length > 0) {
        setSpeedTestResults((prev) => ({
          ...prev,
          [provider.id]: results[0],
        }));

        if (results[0].error) {
          onNotify?.(
            `${t("provider.speedTestFailed")}: ${results[0].error}`,
            "error",
            4000
          );
        }
      }
    } catch (error: any) {
      console.error("测速失败:", error);
      onNotify?.(
        error?.message || t("provider.speedTestFailed"),
        "error",
        3000
      );
    } finally {
      setTestingProviderId(null);
    }
  };

  // 处理余额查询 (仅针对 Droid)
  const handleCheckBalance = async (provider: Provider) => {
    if (appType !== "droid") return;
    
    console.log("[Droid Balance] Provider:", provider);
    console.log("[Droid Balance] settingsConfig:", provider.settingsConfig);
    
    const apiKey = provider.settingsConfig?.apiKey;
    console.log("[Droid Balance] API Key:", apiKey);
    
    if (!apiKey) {
      console.error("[Droid Balance] API Key 不存在");
      setBalanceErrors((prev) => ({
        ...prev,
        [provider.id]: "Missing API Key",
      }));
      return;
    }

    setCheckingBalance(provider.id);
    setBalanceErrors((prev) => {
      const newErrors = { ...prev };
      delete newErrors[provider.id];
      return newErrors;
    });

    try {
      console.log("[Droid Balance] 开始查询...");
      const balance = await window.api.checkDroidBalance(apiKey);
      console.log("[Droid Balance] 查询成功:", balance);
      
      setBalances((prev) => ({
        ...prev,
        [provider.id]: balance,
      }));
      setBalanceLastChecked((prev) => ({
        ...prev,
        [provider.id]: Date.now(),
      }));
    } catch (error: any) {
      console.error("[Droid Balance] 查询失败:", error);
      console.error("[Droid Balance] 错误详情:", {
        message: error?.message,
        stack: error?.stack,
        raw: error,
      });
      
      const errorMsg = error?.message || error?.toString() || "Unknown error";
      setBalanceErrors((prev) => ({
        ...prev,
        [provider.id]: errorMsg,
      }));
    } finally {
      setCheckingBalance(null);
    }
  };

  // 批量刷新所有Droid provider的余额
  const handleRefreshAllBalances = useCallback(async () => {
    if (appType !== "droid") return;

    const droidProviders = Object.values(providers).filter(
      p => p.settingsConfig?.apiKey
    );

    if (droidProviders.length === 0) {
      console.log('[ProviderList] 没有配置API Key的provider');
      return;
    }

    console.log(`[ProviderList] 批量刷新 ${droidProviders.length} 个provider的余额（使用批量API，限流200ms/个）`);

    // 收集所有API Keys，并建立 apiKey -> providerId 的映射
    const apiKeyToProviderId: Record<string, string> = {};
    const apiKeys: string[] = [];

    droidProviders.forEach(provider => {
      const apiKey = provider.settingsConfig?.apiKey;
      if (apiKey) {
        apiKeys.push(apiKey);
        apiKeyToProviderId[apiKey] = provider.id;
      }
    });

    try {
      // 使用后端的批量查询API（内置200ms限流）
      const results = await window.api.batchCheckDroidBalances(apiKeys);
      console.log(`[ProviderList] 批量查询完成，成功 ${Object.keys(results).length}/${apiKeys.length} 个`);

      // 更新每个provider的余额
      Object.entries(results).forEach(([apiKey, balance]) => {
        const providerId = apiKeyToProviderId[apiKey];
        if (providerId) {
          setBalances((prev) => ({
            ...prev,
            [providerId]: balance,
          }));
          setBalanceLastChecked((prev) => ({
            ...prev,
            [providerId]: Date.now(),
          }));
          // 清除错误
          setBalanceErrors((prev) => {
            const newErrors = { ...prev };
            delete newErrors[providerId];
            return newErrors;
          });
        }
      });

      // 检查是否有失败的
      const failedKeys = apiKeys.filter(key => !results[key]);
      if (failedKeys.length > 0) {
        console.warn(`[ProviderList] ${failedKeys.length} 个provider查询失败`);
        failedKeys.forEach(apiKey => {
          const providerId = apiKeyToProviderId[apiKey];
          if (providerId) {
            setBalanceErrors((prev) => ({
              ...prev,
              [providerId]: "查询失败",
            }));
          }
        });
      }
    } catch (error: any) {
      console.error('[ProviderList] 批量查询余额失败:', error);
      // 为所有provider设置错误
      droidProviders.forEach(provider => {
        setBalanceErrors((prev) => ({
          ...prev,
          [provider.id]: error?.message || "批量查询失败",
        }));
      });
    }
  }, [appType, providers]);

  // 统一的自动刷新Hook（只在Droid应用中启用）
  const { resetErrors } = useAutoRefresh({
    enabled: appType === "droid" && autoRefreshEnabled,
    interval: refreshInterval,
    onlyWhenVisible: refreshOnlyWhenVisible,
    onRefresh: handleRefreshAllBalances,
    onError: (error) => {
      console.error('[ProviderList] 自动刷新错误:', error);
    },
    storageKey: 'droid-balance-refresh-time', // 使用独立的key
  });

  // 根据延迟返回颜色类
  const getLatencyColor = (latency: number | null): string => {
    if (!latency) return "text-gray-500";
    if (latency < 200) return "text-green-500";
    if (latency < 500) return "text-yellow-500";
    return "text-red-500";
  };

  // 对供应商列表进行排序
  const sortedProviders = Object.values(providers).sort((a, b) => {
    // 按添加时间排序
    // 没有时间戳的视为最早添加的（排在最前面）
    // 有时间戳的按时间升序排列
    const timeA = a.createdAt || 0;
    const timeB = b.createdAt || 0;

    // 如果都没有时间戳，按名称排序
    if (timeA === 0 && timeB === 0) {
      return a.name.localeCompare(b.name, "zh-CN");
    }

    // 如果只有一个没有时间戳，没有时间戳的排在前面
    if (timeA === 0) return -1;
    if (timeB === 0) return 1;

    // 都有时间戳，按时间升序
    return timeA - timeB;
  });

  return (
    <div className="space-y-4">
      {sortedProviders.length === 0 ? (
        <div className="text-center py-12">
          <div className="w-16 h-16 mx-auto mb-4 bg-gray-100 rounded-full flex items-center justify-center">
            <Users size={24} className="text-gray-400" />
          </div>
          <h3 className="text-lg font-medium text-gray-900 dark:text-gray-100 mb-2">
            {t("provider.noProviders")}
          </h3>
          <p className="text-gray-500 dark:text-gray-400 text-sm">
            {t("provider.noProvidersDescription")}
          </p>
        </div>
      ) : (
        <div className="space-y-3">
          {sortedProviders.map((provider) => {
            const isCurrent = provider.id === currentProviderId;
            const apiUrl = getApiUrl(provider);

            return (
              <div
                key={provider.id}
                className={cn(
                  isCurrent ? cardStyles.selected : cardStyles.interactive,
                )}
              >
                <div className="flex items-start justify-between">
                  <div className="flex-1">
                    <div className="flex items-center gap-3 mb-2">
                      <h3 className="font-medium text-gray-900 dark:text-gray-100">
                        {provider.name}
                      </h3>
                      {/* 分类徽章已移除 */}
                      <div
                        className={cn(
                          badgeStyles.success,
                          !isCurrent && "invisible",
                        )}
                      >
                        <CheckCircle2 size={12} />
                        {t("provider.currentlyUsing")}
                      </div>
                    </div>

                    <div className="flex items-center gap-2 text-sm">
                      {provider.websiteUrl ? (
                        <button
                          onClick={(e) => {
                            e.preventDefault();
                            handleUrlClick(provider.websiteUrl!);
                          }}
                          className="inline-flex items-center gap-1 text-blue-500 dark:text-blue-400 hover:opacity-90 transition-colors"
                          title={`访问 ${provider.websiteUrl}`}
                        >
                          {provider.websiteUrl}
                        </button>
                      ) : (
                        <span
                          className="text-gray-500 dark:text-gray-400"
                          title={apiUrl}
                        >
                          {apiUrl}
                        </span>
                      )}
                      {/* 测速结果显示 */}
                      {speedTestResults[provider.id] && (
                        <span
                          className={cn(
                            "inline-flex items-center gap-1 text-xs font-medium",
                            getLatencyColor(speedTestResults[provider.id].latency)
                          )}
                        >
                          <Zap size={12} />
                          {speedTestResults[provider.id].latency ?
                            `${speedTestResults[provider.id].latency}ms` :
                            t("provider.speedTestFailed")}
                        </span>
                      )}
                    </div>
                  </div>

                  <div className="flex items-center gap-2 ml-4">
                    {/* 同步按钮占位容器 - 只在对应模式下渲染，避免布局跳动 */}
                    {appType === "codex" ? (
                      <div className="w-[130px]">
                        {provider.category !== "official" && isCurrent && (
                          <button
                            onClick={() =>
                              vscodeAppliedFor === provider.id
                                ? handleRemoveFromVSCode()
                                : handleApplyToVSCode(provider)
                            }
                            className={cn(
                              "inline-flex items-center gap-1 px-3 py-1.5 text-sm font-medium rounded-md transition-colors w-full whitespace-nowrap justify-center",
                              vscodeAppliedFor === provider.id
                                ? "border border-gray-300 text-gray-600 hover:border-red-300 hover:text-red-600 hover:bg-red-50 dark:border-gray-600 dark:text-gray-400 dark:hover:border-red-800 dark:hover:text-red-400 dark:hover:bg-red-900/20"
                                : "border border-gray-300 text-gray-700 hover:border-blue-300 hover:text-blue-600 hover:bg-blue-50 dark:border-gray-600 dark:text-gray-300 dark:hover:border-blue-700 dark:hover:text-blue-400 dark:hover:bg-blue-900/20",
                            )}
                            title={
                              vscodeAppliedFor === provider.id
                                ? t("provider.removeFromVSCode")
                                : t("provider.applyToVSCode")
                            }
                          >
                            {vscodeAppliedFor === provider.id
                              ? t("provider.removeFromVSCode")
                              : t("provider.applyToVSCode")}
                          </button>
                        )}
                      </div>
                    ) : null}

                    {appType === "claude" ? (
                      <div className="w-[130px]">
                        {provider.category !== "official" && isCurrent && (
                          <button
                            onClick={() =>
                              claudeApplied
                                ? handleRemoveFromClaudePlugin()
                                : handleApplyToClaudePlugin()
                            }
                            className={cn(
                              "inline-flex items-center gap-1 px-3 py-1.5 text-sm font-medium rounded-md transition-colors w-full whitespace-nowrap justify-center",
                              claudeApplied
                                ? "border border-gray-300 text-gray-600 hover:border-red-300 hover:text-red-600 hover:bg-red-50 dark:border-gray-600 dark:text-gray-400 dark:hover:border-red-800 dark:hover:text-red-400 dark:hover:bg-red-900/20"
                                : "border border-gray-300 text-gray-700 hover:border-green-300 hover:text-green-600 hover:bg-green-50 dark:border-gray-600 dark:text-gray-300 dark:hover:border-green-700 dark:hover:text-green-400 dark:hover:bg-green-900/20",
                            )}
                            title={
                              claudeApplied
                                ? t("provider.removeFromClaudePlugin")
                                : t("provider.applyToClaudePlugin")
                            }
                          >
                            {claudeApplied
                              ? t("provider.removeFromClaudePlugin")
                              : t("provider.applyToClaudePlugin")}
                          </button>
                        )}
                      </div>
                    ) : null}

                    {/* 测速按钮 */}
                    <button
                      onClick={() => handleTestSpeed(provider)}
                      disabled={testingProviderId === provider.id}
                      className={cn(
                        buttonStyles.icon,
                        testingProviderId === provider.id && "opacity-50 cursor-wait"
                      )}
                      title={t("provider.testSpeedTooltip")}
                    >
                      {testingProviderId === provider.id ? (
                        <Loader2 size={16} className="animate-spin" />
                      ) : (
                        <Zap size={16} />
                      )}
                    </button>

                    {(appType === "claude" || appType === "droid") && isCurrent ? (
                      <button
                        onClick={onDisable}
                        className="inline-flex items-center gap-1.5 px-3 py-1.5 text-sm font-medium rounded-md transition-colors w-[90px] justify-center whitespace-nowrap bg-orange-500 text-white hover:bg-orange-600 dark:bg-orange-600 dark:hover:bg-orange-700"
                      >
                        <Check size={14} />
                        {t("provider.disable")}
                      </button>
                    ) : (
                      <button
                        onClick={() => onSwitch(provider.id)}
                        disabled={isCurrent}
                        className={cn(
                          "inline-flex items-center gap-1.5 px-3 py-1.5 text-sm font-medium rounded-md transition-colors w-[90px] justify-center whitespace-nowrap",
                          isCurrent
                            ? "bg-gray-100 text-gray-400 dark:bg-gray-800 dark:text-gray-500 cursor-not-allowed"
                            : "bg-blue-500 text-white hover:bg-blue-600 dark:bg-blue-600 dark:hover:bg-blue-700",
                        )}
                      >
                        {isCurrent ? <Check size={14} /> : <Play size={14} />}
                        {isCurrent ? t("provider.inUse") : t("provider.enable")}
                      </button>
                    )}

                    <button
                      onClick={() => onEdit(provider.id)}
                      className={buttonStyles.icon}
                      title={t("provider.editProvider")}
                    >
                      <Edit3 size={16} />
                    </button>

                    <button
                      onClick={() => onDelete(provider.id)}
                      disabled={isCurrent}
                      className={cn(
                        buttonStyles.icon,
                        isCurrent
                          ? "text-gray-400 cursor-not-allowed"
                          : "text-gray-500 hover:text-red-500 hover:bg-red-100 dark:text-gray-400 dark:hover:text-red-400 dark:hover:bg-red-500/10",
                      )}
                      title={t("provider.deleteProvider")}
                    >
                      <Trash2 size={16} />
                    </button>
                  </div>
                </div>

                {/* Droid 余额显示 */}
                {appType === "droid" && (
                  <BalanceDisplay
                    apiKey={provider.settingsConfig?.apiKey || ""}
                    balance={balances[provider.id]}
                    lastChecked={balanceLastChecked[provider.id]}
                    onRefresh={() => handleCheckBalance(provider)}
                    isLoading={checkingBalance === provider.id}
                    error={balanceErrors[provider.id]}
                  />
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
};

export default ProviderList;
