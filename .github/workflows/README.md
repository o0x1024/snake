# GitHub Actions 工作流文档

## 多平台构建工作流

本项目包含三个GitHub Actions工作流，用于自动化多平台构建和发布：

### 1. 测试工作流 (test.yml)
**触发条件**: 每次push到main/develop分支、创建tag(v*格式)或创建PR时

**功能**:
- 在macOS、Windows、Linux上运行测试
- 代码格式检查 (cargo fmt)
- 代码质量检查 (cargo clippy)
- 单元测试 (cargo test)
- 调试构建验证

### 2. 构建工作流 (build.yml)
**触发条件**: push到main/develop分支、创建tag(v*格式)、创建PR、发布release时

**功能**:
- 构建以下平台的应用包：
  - macOS (x64 和 ARM64)
  - Windows (x64)
  - Linux (x64)
- 自动创建Universal macOS二进制文件
- 上传到GitHub Release

### 3. 发布工作流 (release.yml)
**触发条件**: 手动触发 (workflow_dispatch)

**功能**:
- 自动版本管理
- 创建GitHub Release
- 构建并上传所有平台的发布包

## 配置步骤

### 1. 设置Tauri签名密钥

#### 生成私钥
```bash
cd src-tauri
tauri signer generate --password your-secure-password
```

#### 添加密钥到GitHub Secrets
在GitHub仓库设置中添加以下secrets：
- `TAURI_PRIVATE_KEY`: 生成的私钥内容
- `TAURI_KEY_PASSWORD`: 私钥密码

#### macOS签名必需的secrets（仅macOS构建需要）
- `APPLE_CERTIFICATE`: Base64编码的Apple开发者证书(.p12文件)
- `APPLE_CERTIFICATE_PASSWORD`: 证书密码
- `APPLE_SIGNING_IDENTITY`: 签名身份ID（如"Developer ID Application: Your Name (TEAM_ID)"）
- `APPLE_ID`: Apple开发者账号邮箱
- `APPLE_PASSWORD`: Apple专用密码（App专用密码，非账号密码）
- `APPLE_TEAM_ID`: Apple开发者团队ID

### 2. 配置构建环境

#### macOS构建
- 需要在macOS runner上构建
- 自动支持x64和ARM64架构
- 会生成Universal二进制文件

#### Windows构建
- 使用windows-latest runner
- 构建x64架构应用

#### Linux构建
- 使用ubuntu-22.04 runner
- 自动安装必要的系统依赖

### 3. 使用工作流

#### 自动触发
- 推送到main分支会自动运行测试和构建
- 创建release会自动构建所有平台包

#### 手动触发发布
1. 进入GitHub Actions页面
2. 选择"Release"工作流
3. 点击"Run workflow"
4. 选择版本类型 (patch/minor/major)
5. 确认运行

### 4. 下载构建结果

#### 从GitHub Release下载
- 发布完成后，在GitHub Release页面查看
- 每个平台都有对应的安装包
- macOS用户可选择Universal版本或单独架构版本

#### 从Actions Artifacts下载
- 在Actions页面选择对应的工作流运行
- 在Artifacts部分下载构建结果

## 自定义配置

### 修改目标平台
编辑对应的工作流文件，修改`matrix`部分：

```yaml
strategy:
  matrix:
    include:
      - platform: 'macos-latest'
        args: '--target x86_64-apple-darwin'
      # 添加或删除平台
```

### 修改触发条件
根据需要修改工作流的`on`部分：

```yaml
on:
  push:
    branches: [ main, develop, feature/* ]
  schedule:
    - cron: '0 2 * * 0'  # 每周日凌晨2点运行
```

## macOS签名配置指南

### 1. 获取Apple开发者证书

1. 登录[Apple Developer Portal](https://developer.apple.com)
2. 创建或下载您的开发者证书
3. 将证书导出为.p12格式（包含私钥）

### 2. 准备签名所需信息

#### 获取证书Base64编码
```bash
# 将.p12证书转换为Base64
base64 -i certificate.p12 -o certificate_base64.txt
cat certificate_base64.txt
```

#### 获取签名身份ID
```bash
# 在macOS上运行
security find-identity -v -p codesigning
```

#### 获取团队ID
1. 登录[Apple Developer Portal](https://developer.apple.com)
2. 在Account页面查看Team ID

#### 创建App专用密码
1. 登录[Apple ID账户页面](https://appleid.apple.com)
2. 创建App专用密码用于CI/CD

### 3. 配置GitHub Secrets

在GitHub仓库设置中添加以下secrets：

| Secret名称 | 值说明 |
|-----------|--------|
| `APPLE_CERTIFICATE` | 证书Base64字符串 |
| `APPLE_CERTIFICATE_PASSWORD` | 证书导出密码 |
| `APPLE_SIGNING_IDENTITY` | 签名身份ID |
| `APPLE_ID` | Apple开发者邮箱 |
| `APPLE_PASSWORD` | App专用密码 |
| `APPLE_TEAM_ID` | 开发者团队ID |

## 故障排除

### 常见问题

1. **Linux构建失败**
   - 检查系统依赖是否正确安装
   - 确保package.json中依赖版本兼容

2. **Windows构建失败**
   - 检查路径长度限制
   - 确保所有依赖支持Windows

3. **macOS签名问题**
   - 检查签名证书是否正确配置
   - 验证TAURI_PRIVATE_KEY和TAURI_KEY_PASSWORD
   - 检查证书是否过期，签名身份是否正确

### 调试构建

使用测试工作流进行调试：
```bash
# 本地调试
cd src-tauri
cargo build --release
cargo tauri build
```

## 最佳实践

1. **版本管理**: 使用语义化版本号
2. **分支策略**: 使用main分支作为稳定版本
3. **测试**: 在PR中始终运行测试工作流
4. **发布**: 使用发布工作流创建正式版本
5. **监控**: 定期检查构建状态和性能