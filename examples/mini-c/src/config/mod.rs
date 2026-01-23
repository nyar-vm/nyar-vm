//! Mini C 配置模块

use url::Url;

/// 读取配置
#[derive(Clone, Debug, Default)]
pub struct ReadConfig {
    /// 源文件的 URL
    pub url: Option<Url>,
}

impl ReadConfig {
    /// 创建新的配置
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置 URL
    pub fn with_url(mut self, url: Option<Url>) -> Self {
        self.url = url;
        self
    }
}
