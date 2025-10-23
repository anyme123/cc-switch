# Droid 供应商功能文档

## 📋 功能概述

CC Switch v3.5.3 新增 **Droid** 应用类型，提供完整的 Factory API Key 管理功能。

---

## ✨ 核心特性

### 1. **多密钥管理**
- 添加、编辑、删除多个 Factory API Keys
- 批量导入功能：一次性添加多个 Keys
- 每个 Key 独立命名和管理

### 2. **系统环境变量**
- 自动设置 `Factory_API_Key` 环境变量
- Windows: 注册表 `HKEY_CURRENT_USER\Environment`
- macOS/Linux: Shell 配置文件 (.bashrc, .zshrc, .profile)
- 一键停用清除环境变量

### 3. **实时余额查询**
- 查询单个 Key 的余额信息
- 批量查询多个 Keys（自动限流）
- 可视化进度条（绿/黄/橙/红）
- 显示已用/剩余/总配额

### 4. **系统托盘集成**
- 托盘菜单快速切换 Keys
- 折叠子菜单组织（Claude/Codex/Droid）
- 一步到位切换（供应商+端点）

---

## 🚀 使用指南

### 批量导入 API Keys

1. 切换到 **Droid** 面板
2. 点击 **"批量导入"** 按钮（绿色）
3. 粘贴多个 Keys（支持换行、逗号、分号分隔）
4. 设置名称前缀（可选）
5. 点击 **"批量添加"**

**示例**:
```
fk-key1...
fk-key2...
fk-key3...
```

### 切换 API Key

**方法 1**: 主界面
- 点击供应商的 **"启用"** 按钮

**方法 2**: 托盘菜单
- 右键托盘图标 → Droid ► → 点击要用的 Key

### 查询余额

- 点击供应商卡片的 **"查询余额"** 按钮
- 或点击 🔄 刷新图标

### 停用 Key

- 点击当前激活供应商的 **"停用"** 按钮（橙色）
- 环境变量自动清除

---

## 📊 数据结构

### Provider 配置
```json
{
  "id": "uuid",
  "name": "Key 1",
  "settingsConfig": {
    "apiKey": "fk-..."
  },
  "category": "official",
  "createdAt": 1729700000000
}
```

### 余额信息
```json
{
  "used": 1500000,
  "allowance": 20000000,
  "remaining": 18500000,
  "percentUsed": 7.5,
  "exceeded": false
}
```

---

## 🔍 环境变量验证

### 浏览器控制台（F12）
```javascript
await window.api.getFactoryApiKeyEnv()
```

### Windows PowerShell
```powershell
$env:Factory_API_Key
```

### macOS/Linux
```bash
echo $Factory_API_Key
```

---

## 📝 技术细节

### Factory.ai API
- **端点**: `https://app.factory.ai/api/organization/members/chat-usage`
- **方法**: GET
- **认证**: `Authorization: Bearer {api_key}`
- **Header**: `x-factory-client: web-browser`

### 环境变量位置
- **Windows**: `HKCU:\Environment\Factory_API_Key`
- **macOS/Linux**: `~/.bashrc`, `~/.zshrc`, `~/.profile`

---

## 🎯 最佳实践

1. **批量导入**: 使用批量导入功能快速添加多个 Keys
2. **余额监控**: 定期查询余额，及时切换
3. **备用方案**: 保持多个 Keys，余额不足时快速切换
4. **托盘操作**: 使用托盘菜单快速切换，无需打开主界面

---

**版本**: 3.5.3  
**发布日期**: 2025-10-23


