use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::config::Config;

/// Template manager for multi-language support
pub struct TemplateManager {
    lang: String,
    template_dir: PathBuf,
}

impl TemplateManager {
    /// Create a new template manager with the configured language
    pub fn new() -> Result<Self> {
        let config = Config::load()?;
        let lang = config.user_language.unwrap_or_else(|| "zh".to_string());
        let template_dir = Config::config_dir()?.join("templates").join(&lang);

        Ok(Self {
            lang,
            template_dir,
        })
    }

    /// Create with specific language
    pub fn with_language(lang: &str) -> Result<Self> {
        let template_dir = Config::config_dir()?.join("templates").join(lang);

        Ok(Self {
            lang: lang.to_string(),
            template_dir,
        })
    }

    /// Get current language
    pub fn language(&self) -> &str {
        &self.lang
    }

    /// Load template file
    fn load_template(&self, name: &str) -> Result<String> {
        let path = self.template_dir.join(format!("{}.md", name));

        if path.exists() {
            fs::read_to_string(&path)
                .with_context(|| format!("Failed to read template: {}", path.display()))
        } else {
            // Return built-in default template
            Ok(self.get_builtin_template(name))
        }
    }

    /// Get built-in default templates
    fn get_builtin_template(&self, name: &str) -> String {
        match name {
            "endpoint" => self.builtin_endpoint_template(),
            "error" => self.builtin_error_template(),
            "init" => self.builtin_init_template(),
            _ => String::new(),
        }
    }

    /// Built-in endpoint INFO.md template
    fn builtin_endpoint_template(&self) -> String {
        r#"# {{endpoint_path}}

## Endpoint Information
- **Function Name**: {{function_name}}
- **Path**: {{endpoint_path}}
- **Category**: {{category}}
- **Version**: {{version}}
- **Status**: {{status}}
- **Created At**: {{created_at}}
- **Updated At**: {{updated_at}}

## HTTP Methods

### {{method}}

#### Description
{{description}}

#### Request Parameters
| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
{{request_params}}

#### Request Body
```json
{{request_body}}
```

#### Headers
| Header | Required | Description |
|--------|----------|-------------|
{{headers}}

#### Response
```json
{{response}}
```

#### Error Codes
{{error_codes}}

---

## Change History
- {{created_at}}: Initial creation
"#.to_string()
    }

    /// Built-in error ERROR.md template
    fn builtin_error_template(&self) -> String {
        r#"# {{error_code}} - {{error_name}}

## Basic Information
- **Error Code**: {{error_code}}
- **HTTP Status**: {{http_status}}
- **Created**: {{created_at}}

## Description
{{description}}

## Possible Causes
{{causes}}

## Solutions
{{solutions}}

## Related Endpoints
{{related_endpoints}}

## Change History
- {{created_at}}: Initial definition
"#.to_string()
    }

    /// Built-in init README.md template
    fn builtin_init_template(&self) -> String {
        r#"# API Documentation

## Overview
This repository contains API documentation managed by ARM (API Routes Manager).

## Structure
```
api/
└── {{version}}/
    └── {{category}}/
        └── {{endpoint}}/
            └── INFO.md

error/
└── error-{code}/
    └── ERROR.md
```

## Quick Start
- List versions: `arm list versions`
- List endpoints: `arm list endpoints {{version}}-{{category}}`
- Add endpoint: `arm add-endpoint {{version}}-{{category}} {METHOD} {PATH}`
- Add error: `arm add-error {CODE} {MESSAGE}`

## Current Version
See [VERSION.md](./VERSION.md) for the current API version.
"#.to_string()
    }

    /// Render endpoint INFO.md template
    pub fn render_endpoint(
        &self,
        data: &EndpointTemplateData,
    ) -> Result<String> {
        let template = self.load_template("endpoint")?;
        Ok(self.render_template(&template, &data.to_map()))
    }

    /// Render error ERROR.md template
    pub fn render_error(
        &self,
        data: &ErrorTemplateData,
    ) -> Result<String> {
        let template = self.load_template("error")?;
        Ok(self.render_template(&template, &data.to_map()))
    }

    /// Render init README.md template
    pub fn render_init(
        &self,
        data: &InitTemplateData,
    ) -> Result<String> {
        let template = self.load_template("init")?;
        Ok(self.render_template(&template, &data.to_map()))
    }

    /// Simple template rendering - replace {{key}} with value
    fn render_template(&self, template: &str, vars: &HashMap<String, String>) -> String {
        let mut result = template.to_string();
        for (key, value) in vars {
            result = result.replace(&format!("{{{{{}}}}}" , key), value);
        }
        result
    }

    /// Create default templates in config directory if not exist
    pub fn create_default_templates(&self) -> Result<()> {
        let zh_dir = Config::config_dir()?.join("templates").join("zh");
        let en_dir = Config::config_dir()?.join("templates").join("en");

        fs::create_dir_all(&zh_dir)?;
        fs::create_dir_all(&en_dir)?;

        // Chinese templates
        fs::write(zh_dir.join("endpoint.md"), self.builtin_endpoint_template_zh())?;
        fs::write(zh_dir.join("error.md"), self.builtin_error_template_zh())?;
        fs::write(zh_dir.join("init.md"), self.builtin_init_template_zh())?;

        // English templates
        fs::write(en_dir.join("endpoint.md"), self.builtin_endpoint_template())?;
        fs::write(en_dir.join("error.md"), self.builtin_error_template())?;
        fs::write(en_dir.join("init.md"), self.builtin_init_template())?;

        Ok(())
    }

    /// Chinese endpoint template
    fn builtin_endpoint_template_zh(&self) -> String {
        r#"# {{endpoint_path}}

## 端点信息
- **函数名**: {{function_name}}
- **路径**: {{endpoint_path}}
- **分类**: {{category}}
- **版本**: {{version}}
- **状态**: {{status}}
- **创建时间**: {{created_at}}
- **更新时间**: {{updated_at}}

## HTTP 方法

### {{method}}

#### 描述
{{description}}

#### 请求参数
| 参数名 | 类型 | 必填 | 说明 |
|--------|------|------|------|
{{request_params}}

#### 请求体
```json
{{request_body}}
```

#### 请求头
| 请求头 | 必填 | 说明 |
|--------|------|------|
{{headers}}

#### 响应
```json
{{response}}
```

#### 错误码
{{error_codes}}

---

## 变更历史
- {{created_at}}: 初始创建
"#.to_string()
    }

    /// Chinese error template
    fn builtin_error_template_zh(&self) -> String {
        r#"# {{error_code}} - {{error_name}}

## 基本信息
- **错误码**: {{error_code}}
- **HTTP 状态码**: {{http_status}}
- **创建时间**: {{created_at}}

## 描述
{{description}}

## 可能原因
{{causes}}

## 解决方案
{{solutions}}

## 相关端点
{{related_endpoints}}

## 变更历史
- {{created_at}}: 初始定义
"#.to_string()
    }

    /// Chinese init template
    fn builtin_init_template_zh(&self) -> String {
        r#"# API 文档

## 概述
本仓库使用 ARM (API Routes Manager) 管理的 API 文档。

## 结构
```
api/
└── {{version}}/
    └── {{category}}/
        └── {{endpoint}}/
            └── INFO.md

error/
└── error-{code}/
    └── ERROR.md
```

## 快速开始
- 列出版本: `arm list versions`
- 列出端点: `arm list endpoints {{version}}-{{category}}`
- 添加端点: `arm add-endpoint {{version}}-{{category}} {方法} {路径}`
- 添加错误码: `arm add-error {错误码} {描述}`

## 当前版本
当前 API 版本请查看 [VERSION.md](./VERSION.md)。
"#.to_string()
    }
}

/// Data for endpoint template
pub struct EndpointTemplateData {
    pub function_name: String,
    pub endpoint_path: String,
    pub category: String,
    pub version: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub method: String,
    pub description: String,
    pub request_params: String,
    pub request_body: String,
    pub headers: String,
    pub response: String,
    pub error_codes: String,
}

impl EndpointTemplateData {
    fn to_map(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        map.insert("function_name".to_string(), self.function_name.clone());
        map.insert("endpoint_path".to_string(), self.endpoint_path.clone());
        map.insert("category".to_string(), self.category.clone());
        map.insert("version".to_string(), self.version.clone());
        map.insert("status".to_string(), self.status.clone());
        map.insert("created_at".to_string(), self.created_at.clone());
        map.insert("updated_at".to_string(), self.updated_at.clone());
        map.insert("method".to_string(), self.method.clone());
        map.insert("description".to_string(), self.description.clone());
        map.insert("request_params".to_string(), self.request_params.clone());
        map.insert("request_body".to_string(), self.request_body.clone());
        map.insert("headers".to_string(), self.headers.clone());
        map.insert("response".to_string(), self.response.clone());
        map.insert("error_codes".to_string(), self.error_codes.clone());
        map
    }
}

/// Data for error template
pub struct ErrorTemplateData {
    pub error_code: String,
    pub error_name: String,
    pub http_status: String,
    pub created_at: String,
    pub description: String,
    pub causes: String,
    pub solutions: String,
    pub related_endpoints: String,
}

impl ErrorTemplateData {
    fn to_map(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        map.insert("error_code".to_string(), self.error_code.clone());
        map.insert("error_name".to_string(), self.error_name.clone());
        map.insert("http_status".to_string(), self.http_status.clone());
        map.insert("created_at".to_string(), self.created_at.clone());
        map.insert("description".to_string(), self.description.clone());
        map.insert("causes".to_string(), self.causes.clone());
        map.insert("solutions".to_string(), self.solutions.clone());
        map.insert("related_endpoints".to_string(), self.related_endpoints.clone());
        map
    }
}

/// Data for init template
pub struct InitTemplateData {
    pub version: String,
    pub category: String,
    pub endpoint: String,
}

impl InitTemplateData {
    fn to_map(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        map.insert("version".to_string(), self.version.clone());
        map.insert("category".to_string(), self.category.clone());
        map.insert("endpoint".to_string(), self.endpoint.clone());
        map
    }
}
