import { useState } from "react";
import { useTranslation } from "react-i18next";
import { RefreshCw, AlertTriangle, CheckCircle } from "lucide-react";
import type { BalanceInfo } from "../lib/tauri-api";

interface BalanceDisplayProps {
  apiKey: string;
  balance?: BalanceInfo;
  lastChecked?: number;
  onRefresh?: () => void;
  isLoading?: boolean;
  error?: string;
}

export function BalanceDisplay({
  apiKey,
  balance,
  lastChecked,
  onRefresh,
  isLoading = false,
  error,
}: BalanceDisplayProps) {
  const { t } = useTranslation();

  // 格式化为 M 单位
  const formatM = (num: number) => {
    return (num / 1000000).toFixed(2) + "M";
  };

  // 格式化时间
  const formatTime = (timestamp: number) => {
    const date = new Date(timestamp);
    const now = new Date();
    const diff = now.getTime() - date.getTime();
    const minutes = Math.floor(diff / 60000);

    if (minutes < 1) return t("droid.justNow");
    if (minutes < 60) return t("droid.minutesAgo", { minutes });
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return t("droid.hoursAgo", { hours });
    const days = Math.floor(hours / 24);
    return t("droid.daysAgo", { days });
  };

  // 获取进度条颜色
  const getProgressColor = (percent: number) => {
    if (percent >= 90) return "bg-red-500 dark:bg-red-600";
    if (percent >= 80) return "bg-orange-500 dark:bg-orange-600";
    if (percent >= 50) return "bg-yellow-500 dark:bg-yellow-600";
    return "bg-green-500 dark:bg-green-600";
  };

  // 获取状态颜色
  const getStatusColor = (percent: number, exceeded: boolean) => {
    if (exceeded) return "text-red-600 dark:text-red-400";
    if (percent >= 90) return "text-orange-600 dark:text-orange-400";
    if (percent >= 80) return "text-yellow-600 dark:text-yellow-400";
    return "text-green-600 dark:text-green-400";
  };

  if (error) {
    return (
      <div className="mt-2 p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg">
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-2 text-red-700 dark:text-red-400 text-sm">
            <AlertTriangle size={16} />
            <span>{t("droid.balanceError")}: {error}</span>
          </div>
          {onRefresh && (
            <button
              onClick={onRefresh}
              disabled={isLoading}
              className="p-1 hover:bg-red-100 dark:hover:bg-red-800 rounded transition-colors"
              title={t("droid.refreshBalance")}
            >
              <RefreshCw
                size={14}
                className={isLoading ? "animate-spin" : ""}
              />
            </button>
          )}
        </div>
      </div>
    );
  }

  if (!balance) {
    return (
      <div className="mt-2 p-3 bg-gray-50 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg">
        <div className="flex items-center justify-between">
          <span className="text-sm text-gray-600 dark:text-gray-400">
            {t("droid.noBalance")}
          </span>
          {onRefresh && (
            <button
              onClick={onRefresh}
              disabled={isLoading}
              className="inline-flex items-center gap-1 px-2 py-1 text-xs font-medium text-blue-600 dark:text-blue-400 hover:bg-blue-50 dark:hover:bg-blue-900/20 rounded transition-colors"
            >
              <RefreshCw
                size={14}
                className={isLoading ? "animate-spin" : ""}
              />
              {t("droid.checkBalance")}
            </button>
          )}
        </div>
      </div>
    );
  }

  const percentUsed = balance.percentUsed;
  const exceeded = balance.exceeded;

  return (
    <div className="mt-2 p-3 bg-gray-50 dark:bg-gray-800 border border-gray-200 dark:border-gray-700 rounded-lg space-y-2">
      {/* 标题行 */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium text-gray-700 dark:text-gray-300">
            {t("droid.balance")}
          </span>
          {exceeded ? (
            <span className="inline-flex items-center gap-1 px-2 py-0.5 bg-red-100 dark:bg-red-900/30 text-red-700 dark:text-red-400 text-xs font-medium rounded">
              <AlertTriangle size={12} />
              {t("droid.exceeded")}
            </span>
          ) : (
            <span className="inline-flex items-center gap-1 px-2 py-0.5 bg-green-100 dark:bg-green-900/30 text-green-700 dark:text-green-400 text-xs font-medium rounded">
              <CheckCircle size={12} />
              {t("droid.normal")}
            </span>
          )}
        </div>
        {onRefresh && (
          <button
            onClick={onRefresh}
            disabled={isLoading}
            className="p-1 hover:bg-gray-200 dark:hover:bg-gray-700 rounded transition-colors"
            title={t("droid.refreshBalance")}
          >
            <RefreshCw
              size={14}
              className={isLoading ? "animate-spin text-blue-500" : "text-gray-500 dark:text-gray-400"}
            />
          </button>
        )}
      </div>

      {/* 进度条 */}
      <div>
        <div className="flex items-center justify-between mb-1">
          <span className={`text-xs font-medium ${getStatusColor(percentUsed, exceeded)}`}>
            {percentUsed.toFixed(1)}%
          </span>
          <span className="text-xs text-gray-500 dark:text-gray-400">
            {formatM(balance.remaining)} {t("droid.remaining")}
          </span>
        </div>
        <div className="w-full bg-gray-200 dark:bg-gray-700 rounded-full h-2 overflow-hidden">
          <div
            className={`h-full transition-all duration-300 ${getProgressColor(percentUsed)}`}
            style={{ width: `${Math.min(percentUsed, 100)}%` }}
          />
        </div>
      </div>

      {/* 详细信息 */}
      <div className="grid grid-cols-2 gap-2 text-xs">
        <div>
          <span className="text-gray-500 dark:text-gray-400">{t("droid.used")}: </span>
          <span className="font-medium text-gray-700 dark:text-gray-300">{formatM(balance.used)}</span>
        </div>
        <div>
          <span className="text-gray-500 dark:text-gray-400">{t("droid.allowance")}: </span>
          <span className="font-medium text-gray-700 dark:text-gray-300">{formatM(balance.allowance)}</span>
        </div>
      </div>

      {/* 超额信息 */}
      {exceeded && balance.overage > 0 && (
        <div className="text-xs text-red-600 dark:text-red-400">
          <span className="font-medium">Overage: </span>
          {balance.overage.toLocaleString()} tokens
        </div>
      )}

      {/* 最后更新时间 */}
      {lastChecked && (
        <div className="text-xs text-gray-400 dark:text-gray-500">
          {t("droid.lastUpdated")}: {formatTime(lastChecked)}
        </div>
      )}
    </div>
  );
}





