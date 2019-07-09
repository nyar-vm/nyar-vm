import {defineConfig} from 'vitepress'
import {withMermaid} from 'vitepress-plugin-mermaid'
import valkyrieGrammar from './valkyrie.tmLanguage.json' with {type: 'json'}


const config = defineConfig({
    title: 'VOS Language',
    description: 'VOS - A modern RPC schema definition language',

    markdown: {
        theme: {
            light: 'one-light',
            dark: 'one-dark-pro'
        },
        shikiSetup(shiki) {
            shiki.loadLanguageSync({
                name: 'valkyrie',
                scopeName: 'source.valkyrie',
                fileTypes: ['valkyrie'],
                patterns: valkyrieGrammar.patterns,
                repository: valkyrieGrammar.repository
            })
        }
    },
    themeConfig: {
        nav: [
            {text: '首页', link: '/'},
            {
                text: '快速开始',
                items: [
                    {text: '快速开始', link: '/guide/'},
                    {text: '开发指南', link: '/development/'},
                    {text: '维护指南', link: '/maintenance/'},
                    {text: '查看示例', link: '/examples/'}
                ]
            },
            {
                text: '语言规范',
                items: [
                    {text: '语言规范', link: '/language/'},
                    {text: '语法规范', link: '/language/basics'},
                    {text: '类型规范', link: '/language/types'},
                    {text: '中间件规范', link: '/guide/middleware/'},
                    {text: '配置规范', link: '/guide/config'}
                ]
            },
            {text: '常见问题', link: '/faq'}
        ],

        sidebar: {
            '/guide/': [
                {
                    text: '快速开始',
                    items: [
                        {text: '概述', link: '/guide/'},
                        {text: '认证中间件', link: '/guide/auth'},
                        {text: '授权中间件', link: '/guide/acl'},
                        {text: '中间件规范', link: '/guide/middleware/'},
                        {text: '最佳实践', link: '/guide/best-practices'}
                    ]
                }
            ],
            '/language/': [
                {
                    text: '语言规范',
                    items: [
                        {text: '概述', link: '/language/'},
                        {text: '语法基础', link: '/language/basics'},
                        {text: '类型系统', link: '/language/types'},
                        {text: '服务定义', link: '/language/services'},
                        {text: '装饰器系统', link: '/language/decorators'},
                        {text: '模块系统', link: '/language/modules'},
                        {text: '交互定义', link: '/language/interactions'}
                    ]
                }
            ],
            '/development/': [
                {
                    text: '开发指南',
                    items: [
                        {text: '概述', link: '/development/'}
                    ]
                }
            ],
            '/guide/': [
                {
                    text: '快速开始',
                    items: [
                        {text: '概述', link: '/guide/'},
                        {text: '认证中间件', link: '/guide/auth'},
                        {text: '授权中间件', link: '/guide/acl'},
                        {text: '中间件规范', link: '/guide/middleware/'},
                        {text: '最佳实践', link: '/guide/best-practices'},
                        {text: 'Valkyrie Language Features', link: '/guide/valkyrie-language-features'}
                    ]
                }
            ],
            '/maintenance/': [
                {
                    text: '维护指南',
                    items: [
                        {text: '概述', link: '/maintenance/'},
                        {text: 'Project Architecture', link: '/maintenance/project-architecture'},
                        {text: 'Salsa Incremental Compilation', link: '/maintenance/salsa-incremental'},
                        {text: 'Miette Error Handling', link: '/maintenance/miette-error-handling'},
                        {text: 'Execution Models', link: '/maintenance/execution-models'}
                    ]
                }
            ],
            '/examples/': [
                {
                    text: 'Examples',
                    items: [
                        {text: 'Overview', link: '/examples/'},
                        {text: 'E-commerce API', link: '/examples/ecommerce'},
                        {text: 'User Service', link: '/examples/user-service'}
                    ]
                }
            ]
        },

        socialLinks: [
            {icon: 'github', link: 'https://github.com/vos-lang/vos'}
        ],

        footer: {
            message: 'Released under the MIT License.',
            copyright: 'Copyright © 2024 VOS Team'
        }
    },
})


export default withMermaid({
    ...config,
    mermaid: {
        // refer https://mermaid.js.org/config/setup/modules/mermaidAPI.html#mermaidapi-configuration-defaults for options
    },
    // optionally set additional config for plugin itself with MermaidPluginConfig
    mermaidPlugin: {
        class: "mermaid my-class", // set additional css classes for parent container
    },
});