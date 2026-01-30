# Workspace 模式

## 概述

Workspace 模式是 Valkyrie 语言的多项目管理模式，通过 `legions.json` 文件定义工作区配置。当项目根目录存在 `legions.json` 文件时，该目录被识别为 workspace 模式。

## 文件结构

```
workspace-root/
├── legions.json          # Workspace 配置文件
├── project-a/
│   ├── legion.json       # 项目 A 配置
│   ├── library/
│   │   └── _.vk         # 建议：命名空间根目录定义
│   └── binary/
│       └── main.vk      # 注册为指令 `main`
├── project-b/
│   ├── legion.json       # 项目 B 配置
│   ├── library/
│   │   └── _.vk         # 库代码目录
│   └── binary/
│       ├── tool1.vk     # 注册为指令 `tool1`
│       └── tool2/
│           └── _.vk     # 注册为指令 `tool2`
└── shared/
    └── common/
        └── _.vk
```

## legions.json 配置

`legions.json` 是 JSON5 格式的配置文件，支持注释和更灵活的语法：

```json5
{
    // Workspace 基本信息
    "name": "valkyrie-workspace",
    "version": "1.0.0",
    "description": "Valkyrie language workspace",
    
    // 私有工作区标识
    "private": true,
    
    // 成員項目列表
    "members": [
        "projects/*",
        "tools/build-tools"
    ],
    
    // 排除的目录
    "exclude": [
        "legacy/*",
        "experiments/*",
        "temp/*"
    ],
    
    // 默认成员（用于快速构建）
    "default-members": [
        "projects/valkyrie-core",
        "projects/valkyrie-std"
    ],
    
    // Workspace 级别的脚本
    "scripts": {
        "build": "legion build --release",
        "test": "legion test --release",
        "fmt": "legion fmt --all",
        "clean": "legion clean",
        "publish": "git push && git push --tags --prune",
        "upgrade": "legion upgrade --workspace"
    },
    
    // 共享依赖配置
    "dependencies": {
        "shared": {
            "serde": "^1.0",
            "tokio": "^1.0"
        }
    },
    
    // 构建配置
    "build": {
        "profile": {
            "release": {
                "lto": true,
                "opt-level": "s"
            }
        }
    }
}
```

## 语义特性

### 1. 项目发现

- **自动发现**：根据 `members` 模式自动发现子项目
- **显式排除**：通过 `exclude` 排除不需要的目录

### 2. 属性继承 (Property Inheritance)

Workspace 支持自动属性继承机制。如果成员项目未显式定义某些元数据，它们将自动从工作区根目录的 `legions.json` 中继承。

#### 工作区配置 (`legions.json`)
```json5
{
    "name": "my-workspace",
    "version": "1.2.3",
    "description": "A unified workspace",
    "authors": ["Valkyrie Team"],
    "license": "MIT",
    "members": ["projects/*"],
    "dependencies": {
        "shared_lib": "1.0.0"
    }
}
```

#### 成员项目配置 (`projects/my-pkg/legion.json`)
通用元数据（如版本、作者、许可证等）是**自动继承**的，无需额外配置：

```json5
{
    "name": "my-pkg",
    // version 自动继承为 1.2.3
    // description 自动继承为 "A unified workspace"
    
    "dependencies": {
        // 依赖项不会自动继承，必须显式声明
        // 如果要使用工作区定义的版本，直接设为 true 即可
        "shared_lib": true
    }
}
```

自动继承的属性包括：
- `version`
- `description`
- `authors`
- `license`
- `repository`
- `homepage`
- `edition`

### 3. 依赖共享

- **直接引用**：工作区内的包可以通过名称直接 `using` 彼此。
- **依赖声明**：虽然依赖版本可以从工作区继承，但每个包仍需在其 `legion.json` 中显式列出它所使用的依赖项。
- **依赖提升**：在 `legions.json` 中定义的依赖作为共享池，成员项目通过 `true` 引用。
- **递归搜索**：支持通配符模式进行递归项目发现

### 2. 依赖管理

- **共享依赖**：在 workspace 级别定义共享的依赖版本
- **版本统一**：确保所有成员项目使用一致的依赖版本
- **依赖解析**：优化依赖解析和构建缓存

### 3. 构建协调

- **并行构建**：支持成员项目的并行构建
- **增量构建**：智能检测变更，只构建必要的项目
- **构建顺序**：根据依赖关系自动确定构建顺序

### 4. 脚本执行

- **Workspace 脚本**：在 workspace 级别执行的脚本命令
- **批量操作**：对所有成员项目执行相同操作
- **条件执行**：根据项目状态条件性执行脚本

## 项目间依赖

### 内部依赖

```json5
// project-a/legion.json
{
    "dependencies": {
        "project-b": { "path": "../project-b" },
        "shared-utils": true
    }
}
```

### 依赖解析规则

1. **路径依赖**：使用相对路径引用其他成员项目
2. **Workspace 依赖**：使用 `true` 引用 workspace 级别的依赖
3. **版本约束**：支持版本范围和精确版本约束

## 开发工作流

### 1. 初始化 Workspace

```bash
# 创建新的 workspace
mkdir my-workspace
cd my-workspace

# 初始化 legions.json
echo '{ "private": true, "members": ["projects/*"] }' > legions.json
```

### 2. 添加成员项目

```bash
# 创建新项目
mkdir projects/my-project
cd projects/my-project

# 初始化项目配置
echo '{ "name": "my-project", "version": "0.1.0" }' > legion.json
```

### 3. 构建和测试

```bash
# 构建所有项目
valkyrie build

# 测试所有项目
valkyrie test

# 构建特定项目
valkyrie build --package my-project

# 运行二进制程序
v tool1                    # 运行 binary/tool1.vk
v tool2                    # 运行 binary/tool2/_.vk
```

## 最佳实践

### 1. 项目组织

- **逻辑分组**：按功能或层次组织项目
- **清晰命名**：使用一致的项目命名约定
- **文档完整**：为每个项目提供完整的文档

### 2. 依赖管理

- **版本锁定**：在 workspace 级别锁定关键依赖版本
- **最小依赖**：避免不必要的依赖引入
- **定期更新**：定期更新和审查依赖

### 3. 构建优化

- **缓存利用**：充分利用构建缓存
- **并行构建**：合理配置并行构建参数
- **增量构建**：优化代码结构以支持增量构建

## 工具集成

### IDE 支持

- **项目导入**：IDE 自动识别和导入 workspace 结构
- **代码导航**：跨项目的代码导航和引用查找
- **调试支持**：统一的调试和运行配置

### CI/CD 集成

- **构建矩阵**：支持多项目的构建矩阵
- **测试报告**：聚合的测试结果和覆盖率报告
- **部署协调**：协调多项目的部署流程

## 迁移指南

### 从单项目到 Workspace

1. **创建 legions.json**：在根目录创建 workspace 配置
2. **重组项目结构**：将现有代码移动到子项目目录
3. **更新依赖**：调整项目间的依赖关系
4. **测试构建**：验证新结构的构建和测试

### 兼容性考虑

- **向后兼容**：保持与现有工具链的兼容性
- **渐进迁移**：支持渐进式的项目迁移
- **工具适配**：确保开发工具正确识别新结构