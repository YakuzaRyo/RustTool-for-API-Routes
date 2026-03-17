# AI 命令参考指南

本文档为 AI 助手提供 ARM (API Routes Manager) 工具的完整命令参考。

## 快速开始流程

```bash
# 1. 初始化仓库（两种方式）

# 方式A: 在当前目录初始化
arm init

# 方式B: 在 ~/.arm/<name> 创建仓库（推荐）
arm init --name MyAPI

# 2. 配置项目使用仓库（只需配置一次）
arm config -r MyAPI

# 3. 之后可直接使用命令（无需 -r）
arm registry new -d "初始版本"
arm registry category auth -d "认证接口"
arm registry endpoint auth/users -d "用户管理"
arm registry method auth/users/POST -d "创建用户"
arm registry method auth/users/GET -d "获取用户列表"

# 4. 查看端点（无需 -r）
arm show auth/users/POST

# 5. 创建错误码
arm registry error E001 "用户不存在" --status 404

# 6. 查看错误码
arm show error/E001
```

## 核心概念

- **仓库位置**: `~/.arm/<name>` 或当前目录
- **分支结构**: master → api → v1 → v1-xxxxxx
- **路径格式**: `{version}/{category}/{resource}/{method}`
- **错误码格式**: `error/{code}` (如 `error/E001`)
- **自动解析**: 配置后无需 `-r` 参数，直接使用命令

## 命令速查表

### 初始化与管理

| 命令 | 说明 |
|------|------|
| `arm init` | 在当前目录初始化 ARM 结构 |
| `arm init --name <name>` | 在 ~/.arm/<name> 创建仓库 |
| `arm scan` | 扫描 ~/.arm 下的所有仓库 |
| `arm show-repos` | 显示已记录的仓库列表 |
| `arm show-version` | 显示当前版本和所有端点 |
| `arm config -r <name>` | 配置使用指定仓库 |

### 仓库操作

```bash
# 方式1: 在项目目录配置默认仓库（推荐）
cd myproject
arm config -r MyAPI  # 保存到 .arm/repo.json

# 方式2: 直接使用 -r 参数
arm -r MyAPI <command>
arm -r "C:\Users\12846\.arm\MyAPI" <command>

# 配置后可直接使用命令（无需 -r）
arm registry new
arm show v1
arm registry category auth
```

### 版本管理

```bash
# 创建新版本（自动从最新版本递增）
arm registry new
arm registry new -d "版本描述"

# 列出所有版本分支
cd ~/.arm/MyAPI && git branch -a | grep "^  v"
```

### 分类管理

```bash
# 创建分类
arm registry category <name> -d "描述"

# 示例
arm registry category auth -d "认证接口"
arm registry category users -d "用户接口"
arm registry category payment -d "支付接口"
```

### 端点管理

```bash
# 创建端点
arm registry endpoint <category>/<resource> -d "描述"

# 示例
arm registry endpoint auth/login -d "用户登录"
arm registry endpoint users/profile -d "用户资料"
```

### 方法管理

```bash
# 创建方法（自动创建端点）
arm registry method <category>/<resource>/<METHOD> -d "描述"

# METHOD 支持: GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS

# 示例
arm registry method auth/login/POST -d "用户登录"
arm registry method users/profile/GET -d "获取用户资料"
arm registry method users/profile/PUT -d "更新用户资料"
arm registry method users/profile/DELETE -d "删除用户"
```

### 错误码管理

```bash
# 创建错误码
arm registry error <CODE> <MESSAGE> --status <HTTP_STATUS>

# CODE 格式: E001, E002, ... (必须 E 开头 + 3位数字)

# 示例
arm registry error E001 "用户不存在" --status 404
arm registry error E002 "权限不足" --status 403
arm registry error E003 "服务器错误" --status 500
arm registry error E004 "参数错误" --status 400
```

### 查看与更新

```bash
# 查看当前版本和所有端点（仅显示 method）
arm show-version

# 查看端点详情（JSON 格式）
arm show <path>

# 示例
arm show auth/login/POST
arm show users/profile/GET

# 查看错误码详情
arm show error/E001

# 更新信息
arm update <path> "key:value"

# 可用 key:
# - description: 描述
# - status: 状态 (active/deprecated)

# 示例
arm update auth/login/POST "description:用户登录接口"
arm update auth/login/POST "status:deprecated"
arm update error/E001 "description:用户ID不存在"
arm update error/E001 "message:用户不存在，请检查ID"
```

**show-version 输出示例：**
```
→ Current Version: v1

Description: v1

→ Endpoints:

  auth/users/GET
  auth/login/POST
```

### 挂载与检查

```bash
# 挂载已有仓库
arm mount <path>

# 检查仓库是否符合 ARM 要求
arm check [path]

# 示例
arm check ~/.arm/MyAPI
arm check /path/to/repo
```

## 工作目录优先级

当运行命令时，仓库路径的优先级：

1. **`-r` / `--repo` 参数** (最高)
   ```bash
   arm -r MyAPI show v1
   arm -r "C:\Users\11846\.arm\MyAPI" show v1
   ```

2. **本地配置** `.arm/repo.json` → 全局 `repos.json`
   ```bash
   # 在项目目录配置
   cd myproject
   arm config -r MyAPI  # 保存到 .arm/repo.json

   # 之后自动使用
   arm show v1  # 自动使用 MyAPI
   ```

3. **当前目录** (默认)

## 仓库路径解析

```bash
# repos.json 存储位置：与 arm.exe 同级目录
# 内容格式：
# {
#   "repos": [
#     {"name": "MyAPI", "path": "C:\\Users\\11846\\.arm\\MyAPI"},
#     {"name": "RealGateWay", "path": "C:\\Users\\11846\\.arm\\RealGateWay"}
#   ]
# }

# 配置后直接使用命令，无需 -r
# 系统自动从 repos.json 查找仓库路径
```

## 常见工作流

### AI 辅助 API 文档管理

```bash
# 1. 创建仓库
arm init --name MyAPI

# 2. 配置默认仓库（在项目目录）
arm config -r MyAPI

# 3. 创建版本
arm registry new -d "V1 API"

# 4. AI 可以批量创建分类
arm registry category auth -d "认证相关"
arm registry category users -d "用户管理"
arm registry category products -d "产品管理"
arm registry category orders -d "订单管理"

# 5. AI 创建端点和方法
arm registry method auth/login/POST -d "用户登录"
arm registry method users/list/GET -d "获取用户列表"
arm registry method users/create/POST -d "创建用户"

# 6. AI 创建错误码
arm registry error E001 "请求参数错误" --status 400
arm registry error E002 "未授权" --status 401
arm registry error E003 "资源不存在" --status 404

# 7. 查看和更新
arm show auth/login/POST
arm update auth/login/POST "description:处理用户登录请求"
```

### 项目集成

```bash
# 在项目目录集成 ARM
cd my-api-project

# 配置默认仓库（推荐）
arm config -r MyAPI

# 之后直接使用命令
arm show auth/login/POST
arm registry new
```

## Git 分支结构

```
master                 # 映射文件分支
├── api                # API 根分支
│   └── v1             # 版本分支
│       ├── v1-abc123  # 分类/端点/方法分支
├── error              # 错误码根分支
│   └── error-E001     # 错误码分支
```

## AI 提示词模板

```
请帮我使用 ARM 工具管理 API 文档：
1. 在 MyAPI 仓库创建版本 v2
2. 创建分类 "products" 用于产品管理
3. 创建端点 products/list 用于产品列表
4. 创建方法 products/list/GET 获取产品列表
5. 创建错误码 E101 产品不存在 404
6. 查看刚创建的端点详情
```
