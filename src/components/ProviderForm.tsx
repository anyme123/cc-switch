import React, { useState, useEffect } from "react";
import { Provider, ProviderCategory } from "../types";
import { AppType } from "../lib/tauri-api";
import {
  updateCoAuthoredSetting,
  checkCoAuthoredSetting,
  getApiKeyFromConfig,
  hasApiKeyField,
  setApiKeyInConfig,
} from "../utils/providerConfigUtils";
import { providerPresets } from "../config/providerPresets";
import { codexProviderPresets } from "../config/codexProviderPresets";
import PresetSelector from "./ProviderForm/PresetSelector";
import ApiKeyInput from "./ProviderForm/ApiKeyInput";
import ClaudeConfigEditor from "./ProviderForm/ClaudeConfigEditor";
import CodexConfigEditor from "./ProviderForm/CodexConfigEditor";
import KimiModelSelector from "./ProviderForm/KimiModelSelector";
import { X, AlertCircle, Save } from "lucide-react";
// 分类仅用于控制少量交互（如官方禁用 API Key），不显示介绍组件

interface ProviderFormProps {
  appType?: AppType;
  title: string;
  submitText: string;
  initialData?: Provider;
  showPresets?: boolean;
  onSubmit: (data: Omit<Provider, "id">) => void;
  onClose: () => void;
}

const ProviderForm: React.FC<ProviderFormProps> = ({
  appType = "claude",
  title,
  submitText,
  initialData,
  showPresets = false,
  onSubmit,
  onClose,
}) => {
  // 对于 Codex，需要分离 auth 和 config
  const isCodex = appType === "codex";

  const [formData, setFormData] = useState({
    name: initialData?.name || "",
    websiteUrl: initialData?.websiteUrl || "",
    settingsConfig: initialData
      ? JSON.stringify(initialData.settingsConfig, null, 2)
      : "",
  });
  const [category, setCategory] = useState<ProviderCategory | undefined>(
    initialData?.category,
  );

  // Claude 模型配置状态
  const [claudeModel, setClaudeModel] = useState("");
  const [claudeSmallFastModel, setClaudeSmallFastModel] = useState("");
  const [baseUrl, setBaseUrl] = useState(""); // 新增：基础 URL 状态

  // Codex 特有的状态
  const [codexAuth, setCodexAuth] = useState("");
  const [codexConfig, setCodexConfig] = useState("");
  const [codexApiKey, setCodexApiKey] = useState("");
  // -1 表示自定义，null 表示未选择，>= 0 表示预设索引
  const [selectedCodexPreset, setSelectedCodexPreset] = useState<number | null>(
    showPresets && isCodex ? -1 : null,
  );

  // 初始化 Codex 配置
  useEffect(() => {
    if (isCodex && initialData) {
      const config = initialData.settingsConfig;
      if (typeof config === "object" && config !== null) {
        setCodexAuth(JSON.stringify(config.auth || {}, null, 2));
        setCodexConfig(config.config || "");
        try {
          const auth = config.auth || {};
          if (auth && typeof auth.OPENAI_API_KEY === "string") {
            setCodexApiKey(auth.OPENAI_API_KEY);
          }
        } catch {
          // ignore
        }
      }
    }
  }, [isCodex, initialData]);

  const [error, setError] = useState("");
  const [disableCoAuthored, setDisableCoAuthored] = useState(false);
  // -1 表示自定义，null 表示未选择，>= 0 表示预设索引
  const [selectedPreset, setSelectedPreset] = useState<number | null>(
    showPresets ? -1 : null,
  );
  const [apiKey, setApiKey] = useState("");

  // Kimi 模型选择状态
  const [kimiAnthropicModel, setKimiAnthropicModel] = useState("");
  const [kimiAnthropicSmallFastModel, setKimiAnthropicSmallFastModel] =
    useState("");

  // 初始化自定义模式的默认配置
  useEffect(() => {
    if (
      showPresets &&
      selectedPreset === -1 &&
      !initialData &&
      formData.settingsConfig === ""
    ) {
      // 设置自定义模板
      const customTemplate = {
        env: {
          ANTHROPIC_BASE_URL: "https://your-api-endpoint.com",
          ANTHROPIC_AUTH_TOKEN: "",
          // 可选配置
          // ANTHROPIC_MODEL: "your-model-name",
          // ANTHROPIC_SMALL_FAST_MODEL: "your-fast-model-name"
        },
      };

      setFormData((prev) => ({
        ...prev,
        settingsConfig: JSON.stringify(customTemplate, null, 2),
      }));
      setApiKey("");
    }
  }, []); // 只在组件挂载时执行一次

  // 初始化时检查禁用签名状态
  useEffect(() => {
    if (initialData) {
      const configString = JSON.stringify(initialData.settingsConfig, null, 2);
      const hasCoAuthoredDisabled = checkCoAuthoredSetting(configString);
      setDisableCoAuthored(hasCoAuthoredDisabled);

      // 初始化模型配置（编辑模式）
      if (
        initialData.settingsConfig &&
        typeof initialData.settingsConfig === "object"
      ) {
        const config = initialData.settingsConfig as {
          env?: Record<string, any>;
        };
        if (config.env) {
          setClaudeModel(config.env.ANTHROPIC_MODEL || "");
          setClaudeSmallFastModel(config.env.ANTHROPIC_SMALL_FAST_MODEL || "");
          setBaseUrl(config.env.ANTHROPIC_BASE_URL || ""); // 初始化基础 URL

          // 初始化 Kimi 模型选择
          setKimiAnthropicModel(config.env.ANTHROPIC_MODEL || "");
          setKimiAnthropicSmallFastModel(
            config.env.ANTHROPIC_SMALL_FAST_MODEL || "",
          );
        }
      }
    }
  }, [initialData]);

  // 当选择预设变化时，同步类别
  useEffect(() => {
    if (!showPresets) return;
    if (!isCodex) {
      if (selectedPreset !== null && selectedPreset >= 0) {
        const preset = providerPresets[selectedPreset];
        setCategory(
          preset?.category || (preset?.isOfficial ? "official" : undefined),
        );
      } else if (selectedPreset === -1) {
        setCategory("custom");
      }
    } else {
      if (selectedCodexPreset !== null && selectedCodexPreset >= 0) {
        const preset = codexProviderPresets[selectedCodexPreset];
        setCategory(
          preset?.category || (preset?.isOfficial ? "official" : undefined),
        );
      } else if (selectedCodexPreset === -1) {
        setCategory("custom");
      }
    }
  }, [showPresets, isCodex, selectedPreset, selectedCodexPreset]);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    setError("");

    if (!formData.name) {
      setError("请填写供应商名称");
      return;
    }

    let settingsConfig: Record<string, any>;

    if (isCodex) {
      // Codex: 仅要求 auth.json 必填；config.toml 可为空
      if (!codexAuth.trim()) {
        setError("请填写 auth.json 配置");
        return;
      }

      try {
        const authJson = JSON.parse(codexAuth);

        // 非官方预设强制要求 OPENAI_API_KEY
        if (selectedCodexPreset !== null) {
          const preset = codexProviderPresets[selectedCodexPreset];
          const isOfficial = Boolean(preset?.isOfficial);
          if (!isOfficial) {
            const key =
              typeof authJson.OPENAI_API_KEY === "string"
                ? authJson.OPENAI_API_KEY.trim()
                : "";
            if (!key) {
              setError("请填写 OPENAI_API_KEY");
              return;
            }
          }
        }

        settingsConfig = {
          auth: authJson,
          config: codexConfig ?? "",
        };
      } catch (err) {
        setError("auth.json 格式错误，请检查JSON语法");
        return;
      }
    } else {
      // Claude: 原有逻辑
      if (!formData.settingsConfig.trim()) {
        setError("请填写配置内容");
        return;
      }

      try {
        settingsConfig = JSON.parse(formData.settingsConfig);
      } catch (err) {
        setError("配置JSON格式错误，请检查语法");
        return;
      }
    }

    onSubmit({
      name: formData.name,
      websiteUrl: formData.websiteUrl,
      settingsConfig,
      // 仅在用户选择了预设或手动选择“自定义”时持久化分类
      ...(category ? { category } : {}),
    });
  };

  const handleChange = (
    e: React.ChangeEvent<HTMLInputElement | HTMLTextAreaElement>,
  ) => {
    const { name, value } = e.target;

    if (name === "settingsConfig") {
      // 同时检查并同步选择框状态
      const hasCoAuthoredDisabled = checkCoAuthoredSetting(value);
      setDisableCoAuthored(hasCoAuthoredDisabled);

      // 同步 API Key 输入框显示与值
      const parsedKey = getApiKeyFromConfig(value);
      setApiKey(parsedKey);

      // 不再从 JSON 自动提取或覆盖官网地址，只更新配置内容
      setFormData((prev) => ({
        ...prev,
        [name]: value,
      }));
    } else {
      setFormData({
        ...formData,
        [name]: value,
      });
    }
  };

  // 处理选择框变化
  const handleCoAuthoredToggle = (checked: boolean) => {
    setDisableCoAuthored(checked);

    // 更新JSON配置
    const updatedConfig = updateCoAuthoredSetting(
      formData.settingsConfig,
      checked,
    );
    setFormData({
      ...formData,
      settingsConfig: updatedConfig,
    });
  };

  const applyPreset = (preset: (typeof providerPresets)[0], index: number) => {
    const configString = JSON.stringify(preset.settingsConfig, null, 2);

    setFormData({
      name: preset.name,
      websiteUrl: preset.websiteUrl,
      settingsConfig: configString,
    });
    setCategory(
      preset.category || (preset.isOfficial ? "official" : undefined),
    );

    // 设置选中的预设
    setSelectedPreset(index);

    // 清空 API Key 输入框，让用户重新输入
    setApiKey("");
    setBaseUrl(""); // 清空基础 URL

    // 同步选择框状态
    const hasCoAuthoredDisabled = checkCoAuthoredSetting(configString);
    setDisableCoAuthored(hasCoAuthoredDisabled);

    // 如果预设包含模型配置，初始化模型输入框
    if (preset.settingsConfig && typeof preset.settingsConfig === "object") {
      const config = preset.settingsConfig as { env?: Record<string, any> };
      if (config.env) {
        setClaudeModel(config.env.ANTHROPIC_MODEL || "");
        setClaudeSmallFastModel(config.env.ANTHROPIC_SMALL_FAST_MODEL || "");

        // 如果是 Kimi 预设，同步 Kimi 模型选择
        if (preset.name?.includes("Kimi")) {
          setKimiAnthropicModel(config.env.ANTHROPIC_MODEL || "");
          setKimiAnthropicSmallFastModel(
            config.env.ANTHROPIC_SMALL_FAST_MODEL || "",
          );
        }
      } else {
        setClaudeModel("");
        setClaudeSmallFastModel("");
      }
    }
  };

  // 处理点击自定义按钮
  const handleCustomClick = () => {
    setSelectedPreset(-1);

    // 设置自定义模板
    const customTemplate = {
      env: {
        ANTHROPIC_BASE_URL: "https://your-api-endpoint.com",
        ANTHROPIC_AUTH_TOKEN: "",
        // 可选配置
        // ANTHROPIC_MODEL: "your-model-name",
        // ANTHROPIC_SMALL_FAST_MODEL: "your-fast-model-name"
      },
    };

    setFormData({
      name: "",
      websiteUrl: "",
      settingsConfig: JSON.stringify(customTemplate, null, 2),
    });
    setApiKey("");
    setBaseUrl("https://your-api-endpoint.com"); // 设置默认的基础 URL
    setDisableCoAuthored(false);
    setClaudeModel("");
    setClaudeSmallFastModel("");
    setKimiAnthropicModel("");
    setKimiAnthropicSmallFastModel("");
    setCategory("custom");
  };

  // Codex: 应用预设
  const applyCodexPreset = (
    preset: (typeof codexProviderPresets)[0],
    index: number,
  ) => {
    const authString = JSON.stringify(preset.auth || {}, null, 2);
    setCodexAuth(authString);
    setCodexConfig(preset.config || "");

    setFormData((prev) => ({
      ...prev,
      name: preset.name,
      websiteUrl: preset.websiteUrl,
    }));

    setSelectedCodexPreset(index);
    setCategory(
      preset.category || (preset.isOfficial ? "official" : undefined),
    );

    // 清空 API Key，让用户重新输入
    setCodexApiKey("");
  };

  // Codex: 处理点击自定义按钮
  const handleCodexCustomClick = () => {
    setSelectedCodexPreset(-1);
    setFormData({
      name: "",
      websiteUrl: "",
      settingsConfig: "",
    });
    setCodexAuth("");
    setCodexConfig("");
    setCodexApiKey("");
    setCategory("custom");
  };

  // 处理 API Key 输入并自动更新配置
  const handleApiKeyChange = (key: string) => {
    setApiKey(key);

    const configString = setApiKeyInConfig(
      formData.settingsConfig,
      key.trim(),
      { createIfMissing: selectedPreset !== null && selectedPreset !== -1 },
    );

    // 更新表单配置
    setFormData((prev) => ({
      ...prev,
      settingsConfig: configString,
    }));

    // 同步选择框状态
    const hasCoAuthoredDisabled = checkCoAuthoredSetting(configString);
    setDisableCoAuthored(hasCoAuthoredDisabled);
  };

  // 处理基础 URL 变化
  const handleBaseUrlChange = (url: string) => {
    setBaseUrl(url);

    try {
      const config = JSON.parse(formData.settingsConfig || "{}");
      if (!config.env) {
        config.env = {};
      }
      config.env.ANTHROPIC_BASE_URL = url.trim();

      setFormData((prev) => ({
        ...prev,
        settingsConfig: JSON.stringify(config, null, 2),
      }));
    } catch {
      // ignore
    }
  };

  // Codex: 处理 API Key 输入并写回 auth.json
  const handleCodexApiKeyChange = (key: string) => {
    setCodexApiKey(key);
    try {
      const auth = JSON.parse(codexAuth || "{}");
      auth.OPENAI_API_KEY = key.trim();
      setCodexAuth(JSON.stringify(auth, null, 2));
    } catch {
      // ignore
    }
  };

  // 根据当前配置决定是否展示 API Key 输入框
  // 自定义模式(-1)也需要显示 API Key 输入框
  const showApiKey =
    selectedPreset !== null ||
    (!showPresets && hasApiKeyField(formData.settingsConfig));

  // 判断当前选中的预设是否是官方
  const isOfficialPreset =
    (selectedPreset !== null &&
      selectedPreset >= 0 &&
      (providerPresets[selectedPreset]?.isOfficial === true ||
        providerPresets[selectedPreset]?.category === "official")) ||
    category === "official";

  // 判断当前选中的预设是否是 Kimi
  const isKimiPreset =
    selectedPreset !== null &&
    selectedPreset >= 0 &&
    providerPresets[selectedPreset]?.name?.includes("Kimi");

  // 判断当前编辑的是否是 Kimi 提供商（通过名称或配置判断）
  const isEditingKimi =
    initialData &&
    (formData.name.includes("Kimi") ||
      formData.name.includes("kimi") ||
      (formData.settingsConfig.includes("api.moonshot.cn") &&
        formData.settingsConfig.includes("ANTHROPIC_MODEL")));

  // 综合判断是否应该显示 Kimi 模型选择器
  const shouldShowKimiSelector = isKimiPreset || isEditingKimi;

  // 判断是否显示基础 URL 输入框（仅自定义模式显示）
  const showBaseUrlInput = selectedPreset === -1 && !isCodex;

  // 判断是否显示"获取 API Key"链接（国产官方、聚合站和第三方显示）
  const shouldShowApiKeyLink =
    !isCodex &&
    !isOfficialPreset &&
    (category === "cn_official" ||
      category === "aggregator" ||
      category === "third_party" ||
      (selectedPreset !== null &&
        selectedPreset >= 0 &&
        (providerPresets[selectedPreset]?.category === "cn_official" ||
          providerPresets[selectedPreset]?.category === "aggregator" ||
          providerPresets[selectedPreset]?.category === "third_party")));

  // 获取当前供应商的网址
  const getCurrentWebsiteUrl = () => {
    if (selectedPreset !== null && selectedPreset >= 0) {
      return providerPresets[selectedPreset]?.websiteUrl || "";
    }
    return formData.websiteUrl || "";
  };

  // 获取 Codex 当前供应商的网址
  const getCurrentCodexWebsiteUrl = () => {
    if (selectedCodexPreset !== null && selectedCodexPreset >= 0) {
      return codexProviderPresets[selectedCodexPreset]?.websiteUrl || "";
    }
    return formData.websiteUrl || "";
  };

  // Codex: 控制显示 API Key 与官方标记
  const getCodexAuthApiKey = (authString: string): string => {
    try {
      const auth = JSON.parse(authString || "{}");
      return typeof auth.OPENAI_API_KEY === "string" ? auth.OPENAI_API_KEY : "";
    } catch {
      return "";
    }
  };

  // 自定义模式(-1)不显示独立的 API Key 输入框
  const showCodexApiKey =
    (selectedCodexPreset !== null && selectedCodexPreset !== -1) ||
    (!showPresets && getCodexAuthApiKey(codexAuth) !== "");

  // 不再渲染分类介绍组件，避免造成干扰

  const isCodexOfficialPreset =
    (selectedCodexPreset !== null &&
      selectedCodexPreset >= 0 &&
      (codexProviderPresets[selectedCodexPreset]?.isOfficial === true ||
        codexProviderPresets[selectedCodexPreset]?.category === "official")) ||
    category === "official";

  // 判断是否显示 Codex 的"获取 API Key"链接
  const shouldShowCodexApiKeyLink =
    isCodex &&
    !isCodexOfficialPreset &&
    (category === "cn_official" ||
      category === "aggregator" ||
      category === "third_party" ||
      (selectedCodexPreset !== null &&
        selectedCodexPreset >= 0 &&
        (codexProviderPresets[selectedCodexPreset]?.category ===
          "cn_official" ||
          codexProviderPresets[selectedCodexPreset]?.category ===
            "aggregator" ||
          codexProviderPresets[selectedCodexPreset]?.category ===
            "third_party")));

  // 处理模型输入变化，自动更新 JSON 配置
  const handleModelChange = (
    field: "ANTHROPIC_MODEL" | "ANTHROPIC_SMALL_FAST_MODEL",
    value: string,
  ) => {
    if (field === "ANTHROPIC_MODEL") {
      setClaudeModel(value);
    } else {
      setClaudeSmallFastModel(value);
    }

    // 更新 JSON 配置
    try {
      const currentConfig = formData.settingsConfig
        ? JSON.parse(formData.settingsConfig)
        : { env: {} };
      if (!currentConfig.env) currentConfig.env = {};

      if (value.trim()) {
        currentConfig.env[field] = value.trim();
      } else {
        delete currentConfig.env[field];
      }

      setFormData((prev) => ({
        ...prev,
        settingsConfig: JSON.stringify(currentConfig, null, 2),
      }));
    } catch (err) {
      // 如果 JSON 解析失败，不做处理
    }
  };

  // Kimi 模型选择处理函数
  const handleKimiModelChange = (
    field: "ANTHROPIC_MODEL" | "ANTHROPIC_SMALL_FAST_MODEL",
    value: string,
  ) => {
    if (field === "ANTHROPIC_MODEL") {
      setKimiAnthropicModel(value);
    } else {
      setKimiAnthropicSmallFastModel(value);
    }

    // 更新配置 JSON
    try {
      const currentConfig = JSON.parse(formData.settingsConfig || "{}");
      if (!currentConfig.env) currentConfig.env = {};
      currentConfig.env[field] = value;

      const updatedConfigString = JSON.stringify(currentConfig, null, 2);
      setFormData((prev) => ({
        ...prev,
        settingsConfig: updatedConfigString,
      }));
    } catch (err) {
      console.error("更新 Kimi 模型配置失败:", err);
    }
  };

  // 初始时从配置中同步 API Key（编辑模式）
  useEffect(() => {
    if (!initialData) return;
    const parsedKey = getApiKeyFromConfig(
      JSON.stringify(initialData.settingsConfig),
    );
    if (parsedKey) setApiKey(parsedKey);
  }, [initialData]);

  // 支持按下 ESC 关闭弹窗
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.preventDefault();
        onClose();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      {/* Backdrop */}
      <div className="absolute inset-0 bg-black/50 dark:bg-black/70 backdrop-blur-sm" />

      {/* Modal */}
      <div className="relative bg-white dark:bg-gray-900 rounded-xl shadow-lg max-w-3xl w-full mx-4 max-h-[90vh] overflow-hidden flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between p-6 border-b border-gray-200 dark:border-gray-800">
          <h2 className="text-xl font-semibold text-gray-900 dark:text-gray-100">
            {title}
          </h2>
          <button
            type="button"
            onClick={onClose}
            className="p-1 text-gray-500 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-100 hover:bg-gray-100 dark:hover:bg-gray-800 rounded-md transition-colors"
            aria-label="关闭"
          >
            <X size={18} />
          </button>
        </div>

        <form onSubmit={handleSubmit} className="flex flex-col flex-1 min-h-0">
          <div className="flex-1 overflow-auto p-6 space-y-6">
            {error && (
              <div className="flex items-center gap-3 p-4 bg-red-100 dark:bg-red-900/20 border border-red-500/20 dark:border-red-500/30 rounded-lg">
                <AlertCircle
                  size={20}
                  className="text-red-500 dark:text-red-400 flex-shrink-0"
                />
                <p className="text-red-500 dark:text-red-400 text-sm font-medium">
                  {error}
                </p>
              </div>
            )}

            {showPresets && !isCodex && (
              <PresetSelector
                presets={providerPresets}
                selectedIndex={selectedPreset}
                onSelectPreset={(index) =>
                  applyPreset(providerPresets[index], index)
                }
                onCustomClick={handleCustomClick}
              />
            )}

            {showPresets && isCodex && (
              <PresetSelector
                presets={codexProviderPresets}
                selectedIndex={selectedCodexPreset}
                onSelectPreset={(index) =>
                  applyCodexPreset(codexProviderPresets[index], index)
                }
                onCustomClick={handleCodexCustomClick}
              />
            )}

            <div className="space-y-2">
              <label
                htmlFor="name"
                className="block text-sm font-medium text-gray-900 dark:text-gray-100"
              >
                供应商名称 *
              </label>
              <input
                type="text"
                id="name"
                name="name"
                value={formData.name}
                onChange={handleChange}
                placeholder="例如：Anthropic 官方"
                required
                autoComplete="off"
                className="w-full px-3 py-2 border border-gray-200 dark:border-gray-700 dark:bg-gray-800 dark:text-gray-100 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500/20 dark:focus:ring-blue-400/20 focus:border-blue-500 dark:focus:border-blue-400 transition-colors"
              />
            </div>

            <div className="space-y-2">
              <label
                htmlFor="websiteUrl"
                className="block text-sm font-medium text-gray-900 dark:text-gray-100"
              >
                官网地址
              </label>
              <input
                type="url"
                id="websiteUrl"
                name="websiteUrl"
                value={formData.websiteUrl}
                onChange={handleChange}
                placeholder="https://example.com（可选）"
                autoComplete="off"
                className="w-full px-3 py-2 border border-gray-200 dark:border-gray-700 dark:bg-gray-800 dark:text-gray-100 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500/20 dark:focus:ring-blue-400/20 focus:border-blue-500 dark:focus:border-blue-400 transition-colors"
              />
            </div>

            {!isCodex && showApiKey && (
              <div className="space-y-1">
                <ApiKeyInput
                  value={apiKey}
                  onChange={handleApiKeyChange}
                  required={!isOfficialPreset}
                  placeholder={
                    isOfficialPreset
                      ? "官方登录无需填写 API Key，直接保存即可"
                      : shouldShowKimiSelector
                        ? "填写后可获取模型列表"
                        : "只需要填这里，下方配置会自动填充"
                  }
                  disabled={isOfficialPreset}
                />
                {shouldShowApiKeyLink && getCurrentWebsiteUrl() && (
                  <div className="-mt-1 pl-1">
                    <a
                      href={getCurrentWebsiteUrl()}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="text-xs text-blue-400 dark:text-blue-500 hover:text-blue-500 dark:hover:text-blue-400 transition-colors"
                    >
                      获取 API Key
                    </a>
                  </div>
                )}
              </div>
            )}

            {/* 基础 URL 输入框 - 仅在自定义模式下显示 */}
            {!isCodex && showBaseUrlInput && (
              <div className="space-y-2">
                <label
                  htmlFor="baseUrl"
                  className="block text-sm font-medium text-gray-900 dark:text-gray-100"
                >
                  请求地址
                </label>
                <input
                  type="url"
                  id="baseUrl"
                  value={baseUrl}
                  onChange={(e) => handleBaseUrlChange(e.target.value)}
                  placeholder="https://your-api-endpoint.com"
                  autoComplete="off"
                  className="w-full px-3 py-2 border border-gray-200 dark:border-gray-700 dark:bg-gray-800 dark:text-gray-100 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500/20 dark:focus:ring-blue-400/20 focus:border-blue-500 dark:focus:border-blue-400 transition-colors"
                />
                <div className="p-3 bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-700 rounded-lg">
                  <p className="text-xs text-amber-600 dark:text-amber-400">
                    💡 填写兼容 Claude API 的服务端点地址
                  </p>
                </div>
              </div>
            )}

            {!isCodex && shouldShowKimiSelector && (
              <KimiModelSelector
                apiKey={apiKey}
                anthropicModel={kimiAnthropicModel}
                anthropicSmallFastModel={kimiAnthropicSmallFastModel}
                onModelChange={handleKimiModelChange}
                disabled={isOfficialPreset}
              />
            )}

            {isCodex && showCodexApiKey && (
              <div className="space-y-1">
                <ApiKeyInput
                  id="codexApiKey"
                  label="API Key"
                  value={codexApiKey}
                  onChange={handleCodexApiKeyChange}
                  placeholder={
                    isCodexOfficialPreset
                      ? "官方无需填写 API Key，直接保存即可"
                      : "只需要填这里，下方 auth.json 会自动填充"
                  }
                  disabled={isCodexOfficialPreset}
                  required={
                    selectedCodexPreset !== null &&
                    selectedCodexPreset >= 0 &&
                    !isCodexOfficialPreset
                  }
                />
                {shouldShowCodexApiKeyLink && getCurrentCodexWebsiteUrl() && (
                  <div className="-mt-1 pl-1">
                    <a
                      href={getCurrentCodexWebsiteUrl()}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="text-xs text-blue-400 dark:text-blue-500 hover:text-blue-500 dark:hover:text-blue-400 transition-colors"
                    >
                      获取 API Key
                    </a>
                  </div>
                )}
              </div>
            )}

            {/* Claude 或 Codex 的配置部分 */}
            {isCodex ? (
              <CodexConfigEditor
                authValue={codexAuth}
                configValue={codexConfig}
                onAuthChange={setCodexAuth}
                onConfigChange={setCodexConfig}
                onAuthBlur={() => {
                  try {
                    const auth = JSON.parse(codexAuth || "{}");
                    const key =
                      typeof auth.OPENAI_API_KEY === "string"
                        ? auth.OPENAI_API_KEY
                        : "";
                    setCodexApiKey(key);
                  } catch {
                    // ignore
                  }
                }}
              />
            ) : (
              <>
                {/* 可选的模型配置输入框 - 仅在非官方且非 Kimi 时显示 */}
                {!isOfficialPreset && !shouldShowKimiSelector && (
                  <div className="space-y-4">
                    <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
                      <div className="space-y-2">
                        <label
                          htmlFor="anthropicModel"
                          className="block text-sm font-medium text-gray-900 dark:text-gray-100"
                        >
                          主模型 (可选)
                        </label>
                        <input
                          type="text"
                          id="anthropicModel"
                          value={claudeModel}
                          onChange={(e) =>
                            handleModelChange("ANTHROPIC_MODEL", e.target.value)
                          }
                          placeholder="例如: GLM-4.5"
                          autoComplete="off"
                          className="w-full px-3 py-2 border border-gray-200 dark:border-gray-700 dark:bg-gray-800 dark:text-gray-100 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500/20 dark:focus:ring-blue-400/20 focus:border-blue-500 dark:focus:border-blue-400 transition-colors"
                        />
                      </div>

                      <div className="space-y-2">
                        <label
                          htmlFor="anthropicSmallFastModel"
                          className="block text-sm font-medium text-gray-900 dark:text-gray-100"
                        >
                          快速模型 (可选)
                        </label>
                        <input
                          type="text"
                          id="anthropicSmallFastModel"
                          value={claudeSmallFastModel}
                          onChange={(e) =>
                            handleModelChange(
                              "ANTHROPIC_SMALL_FAST_MODEL",
                              e.target.value,
                            )
                          }
                          placeholder="例如: GLM-4.5-Air"
                          autoComplete="off"
                          className="w-full px-3 py-2 border border-gray-200 dark:border-gray-700 dark:bg-gray-800 dark:text-gray-100 rounded-lg text-sm focus:outline-none focus:ring-2 focus:ring-blue-500/20 dark:focus:ring-blue-400/20 focus:border-blue-500 dark:focus:border-blue-400 transition-colors"
                        />
                      </div>
                    </div>

                    <div className="p-3 bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-700 rounded-lg">
                      <p className="text-xs text-amber-600 dark:text-amber-400">
                        💡 留空将使用供应商的默认模型
                      </p>
                    </div>
                  </div>
                )}

                <ClaudeConfigEditor
                  value={formData.settingsConfig}
                  onChange={(value) =>
                    handleChange({
                      target: { name: "settingsConfig", value },
                    } as React.ChangeEvent<HTMLTextAreaElement>)
                  }
                  disableCoAuthored={disableCoAuthored}
                  onCoAuthoredToggle={handleCoAuthoredToggle}
                />
              </>
            )}
          </div>

          {/* Footer */}
          <div className="flex items-center justify-end gap-3 p-6 border-t border-gray-200 dark:border-gray-800 bg-gray-100 dark:bg-gray-800">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 text-sm font-medium text-gray-500 dark:text-gray-400 hover:text-gray-900 dark:hover:text-gray-100 hover:bg-white dark:hover:bg-gray-700 rounded-lg transition-colors"
            >
              取消
            </button>
            <button
              type="submit"
              className="px-4 py-2 bg-blue-500 dark:bg-blue-600 text-white rounded-lg hover:bg-blue-600 dark:hover:bg-blue-700 transition-colors text-sm font-medium flex items-center gap-2"
            >
              <Save className="w-4 h-4" />
              {submitText}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};

export default ProviderForm;
