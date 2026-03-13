# API Routes Manager (ARM)

基于 Git 分支的 API 文档和错误码管理工具。

## 功能特性

- 🌳 **Git 分支管理**: 使用 Git 分支组织 API 文档和错误码
- 📝 **结构化文档**: 自动生成 Markdown 格式的端点和错误码文档
- 🔄 **自动版本管理**: 支持创建和管理多个 API 版本
- 🔗 **错误码管理**: 统一管理错误码并关联到 API 端点

## 安装

```bash
cargo build --release
# 编译后的二进制文件位于 target/release/arm.exe (Windows) 或 target/release/arm (Unix)
```

或者将 `arm` 添加到系统 PATH：

```bash
# Windows (PowerShell)
cp target/debug/arm.exe $env:LOCALAPPDATA\Microsoft\WindowsApps\

# 或者添加到 .local/bin
cp target/debug/arm.exe ~/.local/bin/arm
```

## 快速开始

```bash
# 初始化 API 管理结构
arm init

# 创建第一个版本
arm registry new -d "初始版本"

# 创建分类
arm registry category auth -d "认证接口"

# 创建端点
arm registry endpoint auth/users -d "用户管理"

# 创建方法
arm registry method auth/users/POST -d "创建用户"
arm registry method auth/users/GET -d "获取用户"

# 创建错误码
arm registry error E001 "用户不存在" --status 404

# 查看端点详情
arm show auth/users/POST

# 查看错误码详情
arm show error/E001

# 更新端点信息
arm update auth/users/POST "description:创建新用户"

# 更新错误码
arm update error/E001 "description:用户ID不存在"
```

## 命令列表

### init

初始化 API 管理结构（创建 master、api、error 分支）。

```bash
arm init
```

### registry

管理 API 结构的子命令组。

#### new

创建新版本，自动从最新版本递增。

```bash
arm registry new                    # 创建 v1, v2, v3...
arm registry new -d "版本描述"     # 带描述
```

#### category

创建分类。

```bash
arm registry category <name> -d "描述"
# 示例
arm registry category auth -d "认证接口"
arm registry category users -d "用户接口"
```

#### endpoint

创建端点。

```bash
arm registry endpoint <category>/<name> -d "描述"
# 示例
arm registry endpoint auth/login -d "用户登录"
arm registry endpoint users/profile -d "用户资料"
```

#### method

创建方法，会自动创建端点（如果不存在）。

```bash
arm registry method <category>/<resource>/<METHOD> -d "描述"
# METHOD 支持: GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS

# 示例
arm registry method auth/login/POST -d "用户登录"
arm registry method users/profile/GET -d "获取用户资料"
arm registry method users/profile/PUT -d "更新用户资料"
```

#### error

创建错误码。

```bash
arm registry error <CODE> <MESSAGE> --status <HTTP_STATUS>
# CODE 格式: E001, E002, ...

# 示例
arm registry error E001 "用户不存在" --status 404
arm registry error E002 "权限不足" --status 403
arm registry error E003 "服务器错误" --status 500
```

### show

显示端点或错误码的详细信息（JSON 格式）。

```bash
arm show <path>

# 端点示例
arm show auth/users/POST

# 错误码示例
arm show error/E001
```

### update

更新端点或错误码的信息。

```bash
arm update <path> "key:content"

# 可用 key:
# - description: 描述
# - status: 状态 (active/deprecated)
# - message: 错误消息 (仅错误码)

# 端点示例
arm update auth/users/POST "description:创建用户"
arm update auth/users/POST "status:deprecated"

# 错误码示例
arm update error/E001 "description:用户已删除"
arm update error/E001 "message:用户ID不存在"
```

### config

配置 ARM 工具设置。

```bash
arm config --show              # 显示配置
arm config -n "Your Name"     # 设置用户名
arm config -e "your@email.com" # 设置邮箱
arm config -l zh              # 设置语言 (zh/en)
arm config -r /path/to/repo   # 设置仓库路径
arm config --reset            # 重置配置
```

## 全局选项

| 选项 | 说明 |
|------|------|
| `-r, --repo <PATH>` | Git 仓库路径（默认当前目录）|
| `-v, --verbose` | 启用详细输出 |
| `-h, --help` | 显示帮助 |
| `-V, --version` | 显示版本 |

## Git 分支结构

```
master (映射文件)
├── api (API 根分支)
│   └── v1 (版本分支)
│       └── v1-xxxxxx (分类/端点/方法)
├── error (错误码根分支)
│   └── error-xxx (错误码分支)
```

## 工作原理

1. **映射文件**: `.arm/mapping.json` 存储所有路径与分支的映射关系
2. **自动切换**: 所有 registry 命令执行后自动返回 master 分支
3. **版本隔离**: 不同版本 (v1, v2) 相互独立
4. **错误码管理**: 错误码独立于 API 版本

## 依赖

- `clap` - 命令行参数解析
- `git2` - Git 操作
- `colored` - 终端着色
- `regex` - 正则匹配
- `serde` - 序列化
- `chrono` - 日期时间

## 许可证

MIT
