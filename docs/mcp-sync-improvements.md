# MCP 三向同步功能改进

## 修复的问题

### 1. Droid 同步目标硬编码

**之前的问题:**
- Droid 只能同步到 Claude,无法选择同步到 Codex
- 同步目标硬编码在代码中,用户无法选择

**修复后:**
- 添加了同步目标选择器
- 用户可以选择同步到 Claude Code 或 Codex
- 支持三向同步: Claude ↔ Codex ↔ Droid

### 2. 删除 MCP 时 Droid 不同步

**之前的问题:**
```rust
crate::app_config::AppType::Droid => {
    // Droid 暂不支持 MCP 同步
}
```

**修复后:**
```rust
crate::app_config::AppType::Droid => crate::mcp::sync_enabled_to_droid(&cfg2)?,
```

现在删除 Droid MCP 时会自动同步到 `~/.factory/mcp.json`

### 3. UI 显示不完整

**之前的问题:**
- 同步复选框文本只考虑了 Claude 和 Codex
- Droid 的同步目标显示不正确

**修复后:**
- 添加了动态目标选择器
- 根据当前应用类型显示可用的同步目标
- 支持所有三个应用之间的同步

## 新功能

### 三向同步目标选择器

编辑 MCP 时,可以选择同步到其他应用:

```
☑ 同步到 [下拉选择: Claude Code / Codex / Droid]
```

**选择规则:**
- 在 Claude 中: 可选择同步到 Codex 或 Droid
- 在 Codex 中: 可选择同步到 Claude Code 或 Droid
- 在 Droid 中: 可选择同步到 Claude Code 或 Codex

### 默认同步目标

- Claude → Codex (默认)
- Codex → Claude (默认)
- Droid → Claude (默认)

用户可以在 UI 中更改同步目标。

## 使用场景

### 场景 1: 在 Claude 中配置 MCP,同步到 Codex

1. 在 Claude MCP 管理面板中编辑一个 MCP 服务器
2. 勾选"同步到"复选框
3. 选择目标应用: Codex
4. 保存
5. MCP 配置会自动复制到 Codex

### 场景 2: 在 Droid 中配置 MCP,同步到 Claude 和 Codex

1. 在 Droid MCP 管理面板中编辑一个 MCP 服务器
2. 勾选"同步到"复选框
3. 选择目标应用: Claude Code
4. 保存 (同步到 Claude)
5. 再次编辑同一个 MCP
6. 选择目标应用: Codex
7. 保存 (同步到 Codex)

### 场景 3: 处理同步冲突

如果目标应用中已存在同名 MCP:

1. 系统会检测到冲突
2. 弹出确认对话框: "Codex 中已存在同名 MCP 'my-server',是否覆盖?"
3. 用户可以选择:
   - **覆盖**: 用当前配置覆盖目标应用中的配置
   - **跳过**: 保留目标应用中的现有配置

## 技术实现

### 前端改动

**`src/components/mcp/McpFormModal.tsx`:**

1. 添加同步目标状态:
```typescript
const [syncTargetApp, setSyncTargetApp] = useState<string>(() => {
  if (appType === "claude") return "codex";
  if (appType === "codex") return "claude";
  return "claude"; // Droid 默认同步到 Claude
});
```

2. 添加目标选择器 UI:
```tsx
<select
  value={syncTargetApp}
  onChange={(e) => setSyncTargetApp(e.target.value)}
>
  {appType !== "claude" && <option value="claude">Claude Code</option>}
  {appType !== "codex" && <option value="codex">Codex</option>}
  {appType !== "droid" && <option value="droid">Droid</option>}
</select>
```

3. 使用用户选择的目标:
```typescript
const performSync = async (id: string, overwrite: boolean) => {
  const targetApp = syncTargetApp; // 使用用户选择
  await window.api.syncMcpToOtherApp(appType, targetApp, id, overwrite);
};
```

### 后端改动

**`src-tauri/src/commands.rs`:**

修复删除 MCP 时的同步:
```rust
match app_ty {
    crate::app_config::AppType::Claude => crate::mcp::sync_enabled_to_claude(&cfg2)?,
    crate::app_config::AppType::Codex => crate::mcp::sync_enabled_to_codex(&cfg2)?,
    crate::app_config::AppType::Droid => crate::mcp::sync_enabled_to_droid(&cfg2)?, // 新增
}
```

### 国际化

**中文 (`src/i18n/locales/zh.json`):**
```json
{
  "mcp": {
    "sync": {
      "syncTo": "同步到"
    }
  }
}
```

**英文 (`src/i18n/locales/en.json`):**
```json
{
  "mcp": {
    "sync": {
      "syncTo": "Sync to"
    }
  }
}
```

## 同步流程

### 完整同步流程

```
用户编辑 MCP
    ↓
勾选"同步到"
    ↓
选择目标应用 (Claude/Codex/Droid)
    ↓
保存 MCP
    ↓
检查目标应用是否存在同名 MCP
    ↓
    ├─ 不存在 → 直接同步
    └─ 存在 → 显示冲突对话框
              ↓
              ├─ 用户选择"覆盖" → 覆盖同步
              └─ 用户选择"跳过" → 取消同步
```

### 自动同步到配置文件

启用/禁用 MCP 时,会自动同步到对应的配置文件:

- **Claude**: `~/.claude.json`
- **Codex**: `~/.codex/config.toml`
- **Droid**: `~/.factory/mcp.json`

## 已知限制

1. **单次只能同步到一个目标**
   - 如果要同步到多个应用,需要多次操作
   - 未来可以考虑添加"同步到所有应用"功能

2. **同步是单向的**
   - 从源应用复制到目标应用
   - 不会反向同步
   - 不会保持双向同步

3. **新增模式不支持同步**
   - 只有编辑模式才显示同步选项
   - 新增时需要先保存,再编辑并同步

## 测试建议

### 测试用例 1: Claude → Codex 同步

1. 在 Claude 中添加一个 MCP 服务器 "test-server"
2. 编辑 "test-server"
3. 勾选"同步到",选择 Codex
4. 保存
5. 切换到 Codex MCP 面板
6. 验证 "test-server" 已出现

### 测试用例 2: Droid → Claude 同步

1. 在 Droid 中添加一个 MCP 服务器 "droid-server"
2. 编辑 "droid-server"
3. 勾选"同步到",选择 Claude Code
4. 保存
5. 切换到 Claude MCP 面板
6. 验证 "droid-server" 已出现

### 测试用例 3: 同步冲突处理

1. 在 Claude 和 Codex 中都添加名为 "shared-server" 的 MCP
2. 在 Claude 中编辑 "shared-server"
3. 勾选"同步到",选择 Codex
4. 保存
5. 验证弹出冲突对话框
6. 选择"覆盖"
7. 验证 Codex 中的 "shared-server" 被更新

### 测试用例 4: 删除同步

1. 在 Droid 中添加并启用一个 MCP 服务器
2. 验证 `~/.factory/mcp.json` 中包含该服务器
3. 删除该 MCP 服务器
4. 验证 `~/.factory/mcp.json` 中该服务器已被移除

## 相关文件

- `src/components/mcp/McpFormModal.tsx` - MCP 编辑表单 (前端)
- `src-tauri/src/commands.rs` - Tauri 命令处理 (后端)
- `src-tauri/src/mcp.rs` - MCP 同步逻辑 (后端)
- `src/i18n/locales/zh.json` - 中文翻译
- `src/i18n/locales/en.json` - 英文翻译

## 更新日志

- **2025-01-XX**: 添加三向同步目标选择器
- **2025-01-XX**: 修复 Droid 删除时不同步的问题
- **2025-01-XX**: 改进同步 UI,支持动态目标选择

