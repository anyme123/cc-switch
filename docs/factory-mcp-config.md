# Factory MCP 配置说明

## 配置文件位置

Factory AI (Droid) 使用以下路径存储 MCP 配置:

- **Windows**: `C:\Users\<用户名>\.factory\mcp.json`
- **macOS/Linux**: `~/.factory/mcp.json`

## 工作原理

### 1. 自动导入现有配置

当你打开 Droid MCP 管理面板时,应用会:

1. 自动读取 `~/.factory/mcp.json` 文件
2. 将其中的 `mcpServers` 导入到应用内部配置
3. 在 UI 中显示这些服务器
4. 你可以在此基础上添加、编辑、删除服务器

### 2. 双向同步

- **从 Factory → CC Switch**: 启动时自动导入
- **从 CC Switch → Factory**: 启用/禁用服务器时自动同步

### 3. 配置格式

```json
{
  "mcpServers": {
    "server-name": {
      "type": "stdio",
      "command": "node",
      "args": ["/path/to/server.js"]
    }
  }
}
```

## 使用步骤

### 如果你已有 Factory MCP 配置

1. **确认配置文件存在**
   - Windows: 检查 `C:\Users\<你的用户名>\.factory\mcp.json`
   - 运行验证脚本: `scripts\verify-factory-mcp.bat`

2. **启动 CC Switch**
   - 应用会自动读取配置

3. **打开 Droid MCP 管理面板**
   - 应该能看到你现有的 MCP 服务器
   - 可以启用/禁用、编辑、删除

4. **添加新服务器**
   - 点击"添加 MCP"按钮
   - 填写配置
   - 保存后会自动同步到 `~/.factory/mcp.json`

### 如果你还没有配置

1. **在 CC Switch 中添加**
   - 打开 Droid MCP 管理面板
   - 点击"添加 MCP"
   - 填写配置并保存

2. **自动创建配置文件**
   - 应用会自动创建 `~/.factory/mcp.json`
   - 并写入你添加的服务器配置

## 验证配置

### 使用验证脚本

**Windows:**
```
双击运行: scripts\verify-factory-mcp.bat
```

或在 PowerShell 中:
```powershell
.\scripts\verify-factory-mcp.ps1
```

**macOS/Linux:**
```bash
# 检查文件是否存在
ls -la ~/.factory/mcp.json

# 查看内容
cat ~/.factory/mcp.json

# 验证 JSON 格式
cat ~/.factory/mcp.json | python -m json.tool
```

### 手动检查

1. **检查文件是否存在**
   ```powershell
   Test-Path "$env:USERPROFILE\.factory\mcp.json"
   ```

2. **查看文件内容**
   ```powershell
   Get-Content "$env:USERPROFILE\.factory\mcp.json"
   ```

3. **验证 JSON 格式**
   ```powershell
   Get-Content "$env:USERPROFILE\.factory\mcp.json" | ConvertFrom-Json
   ```

## 常见问题

### Q1: 为什么我的配置没有被识别?

**可能原因:**
1. 配置文件不在正确位置 (`~/.factory/mcp.json`)
2. JSON 格式错误
3. 应用没有重启
4. 没有选择 Droid 提供商

**解决方案:**
1. 运行验证脚本检查配置
2. 确保 JSON 格式正确
3. 完全关闭并重启应用
4. 确认已选择 Droid 提供商

### Q2: 配置文件格式是什么?

标准格式:
```json
{
  "mcpServers": {
    "my-server": {
      "type": "stdio",
      "command": "node",
      "args": ["server.js"]
    }
  }
}
```

### Q3: 如何添加多个服务器?

```json
{
  "mcpServers": {
    "server-1": {
      "type": "stdio",
      "command": "node",
      "args": ["server1.js"]
    },
    "server-2": {
      "type": "stdio",
      "command": "python",
      "args": ["-m", "mcp_server"]
    }
  }
}
```

### Q4: 修改配置后需要重启吗?

- **手动修改文件**: 需要重启应用
- **在 UI 中修改**: 不需要重启,自动同步

### Q5: 配置会丢失吗?

不会。配置同时保存在两个地方:
1. `~/.factory/mcp.json` (Factory AI 官方位置)
2. `~/.cc-switch/config.json` (CC Switch 内部配置)

## 技术细节

### 导入逻辑

```rust
// 从 ~/.factory/mcp.json 读取配置
pub fn import_from_droid(config: &mut MultiAppConfig) -> Result<usize, String> {
    let text_opt = crate::factory_mcp::read_mcp_json()?;
    // 解析 JSON
    // 导入到内部配置
    // 设置 enabled=true
}
```

### 同步逻辑

```rust
// 将启用的服务器同步到 ~/.factory/mcp.json
pub fn sync_enabled_to_droid(config: &MultiAppConfig) -> Result<(), String> {
    let enabled = collect_enabled_servers(&config.mcp.droid);
    crate::factory_mcp::set_mcp_servers_map(&enabled)
}
```

## 相关文件

- `src-tauri/src/factory_mcp.rs` - Factory MCP 配置读写
- `src-tauri/src/droid_config.rs` - Droid 配置目录管理
- `src-tauri/src/mcp.rs` - MCP 导入/同步逻辑
- `src/components/mcp/McpPanel.tsx` - MCP 管理 UI

## 更新日志

- **2025-01-XX**: 修正配置路径为 `~/.factory/mcp.json`
- **2025-01-XX**: 添加自动导入功能
- **2025-01-XX**: 添加双向同步功能

