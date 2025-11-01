import { useEffect, useRef, useCallback } from 'react';

interface UseAutoRefreshOptions {
  enabled: boolean;
  interval: number; // 秒
  onlyWhenVisible: boolean;
  onRefresh: () => void | Promise<void>;
  onError?: (error: Error) => void;
  storageKey?: string; // 用于持久化上次刷新时间的key
}

/**
 * 自动刷新Hook
 * - 支持可见性检测
 * - 支持错误处理
 * - 自动清理定时器
 * - 组件重新挂载时保持刷新周期
 */
export function useAutoRefresh({
  enabled,
  interval,
  onlyWhenVisible,
  onRefresh,
  onError,
  storageKey = 'auto-refresh-last-time',
}: UseAutoRefreshOptions) {
  const timerRef = useRef<NodeJS.Timeout | null>(null);
  const consecutiveErrorsRef = useRef(0);
  const isVisibleRef = useRef(true);
  const lastRefreshTimeRef = useRef<number>(0);

  // 从localStorage加载上次刷新时间
  useEffect(() => {
    try {
      const stored = localStorage.getItem(storageKey);
      if (stored) {
        lastRefreshTimeRef.current = parseInt(stored, 10);
        console.log('[AutoRefresh] 加载上次刷新时间:', new Date(lastRefreshTimeRef.current).toLocaleTimeString());
      }
    } catch (error) {
      console.warn('[AutoRefresh] 加载上次刷新时间失败:', error);
    }
  }, [storageKey]);

  // 清理定时器
  const clearTimer = useCallback(() => {
    if (timerRef.current) {
      clearInterval(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  // 执行刷新
  const executeRefresh = useCallback(async () => {
    // 检查是否应该刷新
    if (onlyWhenVisible && !isVisibleRef.current) {
      console.log('[AutoRefresh] 窗口不可见，跳过刷新');
      return;
    }

    try {
      console.log('[AutoRefresh] 执行自动刷新...');
      await onRefresh();
      consecutiveErrorsRef.current = 0;

      // 记录刷新时间
      const now = Date.now();
      lastRefreshTimeRef.current = now;
      try {
        localStorage.setItem(storageKey, now.toString());
      } catch (error) {
        console.warn('[AutoRefresh] 保存刷新时间失败:', error);
      }
    } catch (error) {
      consecutiveErrorsRef.current++;
      console.error(`[AutoRefresh] 刷新失败 (${consecutiveErrorsRef.current}/3):`, error);

      if (onError) {
        onError(error as Error);
      }

      // 连续失败3次后暂停自动刷新
      if (consecutiveErrorsRef.current >= 3) {
        console.warn('[AutoRefresh] 连续失败3次，暂停自动刷新');
        clearTimer();
      }
    }
  }, [onRefresh, onError, onlyWhenVisible, clearTimer, storageKey]);

  // 启动定时器（智能延迟）
  const startTimer = useCallback(() => {
    clearTimer();

    if (!enabled || interval <= 0) {
      return;
    }

    // 计算距离上次刷新过了多久
    const now = Date.now();
    const timeSinceLastRefresh = lastRefreshTimeRef.current
      ? now - lastRefreshTimeRef.current
      : interval * 1000; // 如果没有记录，假设已经过了一个周期

    const timeUntilNextRefresh = Math.max(0, interval * 1000 - timeSinceLastRefresh);

    if (timeUntilNextRefresh > 0) {
      console.log(`[AutoRefresh] 距离上次刷新 ${Math.round(timeSinceLastRefresh / 1000)}秒，${Math.round(timeUntilNextRefresh / 1000)}秒后开始下次刷新`);

      // 使用setTimeout延迟到正确的时间点
      timerRef.current = setTimeout(() => {
        executeRefresh(); // 立即执行一次

        // 然后启动定期刷新
        timerRef.current = setInterval(() => {
          executeRefresh();
        }, interval * 1000);
      }, timeUntilNextRefresh) as any;
    } else {
      console.log(`[AutoRefresh] 启动自动刷新，间隔 ${interval} 秒（立即执行）`);
      executeRefresh(); // 立即执行一次

      // 启动定期刷新
      timerRef.current = setInterval(() => {
        executeRefresh();
      }, interval * 1000);
    }
  }, [enabled, interval, executeRefresh, clearTimer]);

  // 可见性变化处理
  useEffect(() => {
    if (!onlyWhenVisible) {
      return;
    }

    const handleVisibilityChange = () => {
      const isVisible = !document.hidden;
      isVisibleRef.current = isVisible;

      console.log('[AutoRefresh] 可见性变化:', isVisible ? '可见' : '隐藏');

      if (enabled) {
        if (isVisible) {
          // 窗口变为可见时，重新启动定时器
          startTimer();
        } else {
          // 窗口隐藏时，清除定时器
          clearTimer();
        }
      }
    };

    document.addEventListener('visibilitychange', handleVisibilityChange);
    return () => {
      document.removeEventListener('visibilitychange', handleVisibilityChange);
    };
  }, [enabled, onlyWhenVisible, startTimer, clearTimer]);

  // 主效果：启动/停止自动刷新
  useEffect(() => {
    if (enabled) {
      // 检查初始可见性
      isVisibleRef.current = !document.hidden;

      // 如果启用了"仅在可见时刷新"且当前不可见，则不启动
      if (onlyWhenVisible && !isVisibleRef.current) {
        console.log('[AutoRefresh] 窗口不可见，不启动自动刷新');
        return;
      }

      startTimer();
    } else {
      clearTimer();
    }

    return () => {
      clearTimer();
    };
  }, [enabled, interval, onlyWhenVisible, startTimer, clearTimer]);

  // 重置错误计数（当用户手动刷新时调用）
  const resetErrors = useCallback(() => {
    consecutiveErrorsRef.current = 0;
    if (enabled) {
      startTimer();
    }
  }, [enabled, startTimer]);

  return {
    resetErrors,
    clearTimer,
  };
}
