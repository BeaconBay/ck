import { defineConfig } from 'vitepress'

// https://vitepress.dev/reference/site-config
export default defineConfig({
  title: "ck",
  description: "Hybrid Code Search - grep with semantic understanding",
  lang: 'en-US',
  base: '/ck/',

  markdown: {
    typographer: true
  },

  head: [
    ['link', { rel: 'icon', type: 'image/png', href: '/ck/favicon.png' }]
  ],

  themeConfig: {
    // https://vitepress.dev/reference/default-theme-config
    logo: '/logo.png',
    siteTitle: 'ck',

    nav: [
      { text: 'Guide', link: '/guide/introduction' },
      { text: 'Features', link: '/features/semantic-search' },
      { text: 'Reference', link: '/reference/cli' },
      { text: 'Recipes', link: '/recipes/explore-codebase' },
      { text: 'GitHub', link: 'https://github.com/BeaconBay/ck' }
    ],

    sidebar: {
      '/guide/': [
        {
          text: 'Getting Started',
          items: [
            { text: 'Introduction', link: '/guide/introduction' },
            { text: 'Installation', link: '/guide/installation' },
            { text: 'Choosing an Interface', link: '/guide/choosing-interface' },
            { text: 'Basic Usage', link: '/guide/basic-usage' },
            { text: 'Advanced Usage', link: '/guide/advanced-usage' },
            { text: 'AI Agent Setup', link: '/guide/ai-agent-setup' }
          ]
        },
        {
          text: 'Resources',
          items: [
            { text: 'Changelog', link: '/guide/changelog' },
            { text: 'FAQ', link: '/guide/faq' },
            { text: 'Known Limitations', link: '/guide/limitations' },
            { text: 'Roadmap', link: '/guide/roadmap' }
          ]
        }
      ],
      '/recipes/': [
        {
          text: 'Task-Based Guides',
          items: [
            { text: 'Exploring New Codebases', link: '/recipes/explore-codebase' },
            { text: 'Finding Authentication Code', link: '/recipes/find-auth' },
            { text: 'Refactoring Similar Patterns', link: '/recipes/refactor-patterns' },
            { text: 'Security Code Review', link: '/recipes/security-review' },
            { text: 'AI Agent Search Workflows', link: '/recipes/ai-workflows' },
            { text: 'Debugging Production Issues', link: '/recipes/debug-production' }
          ]
        }
      ],
      '/features/': [
        {
          text: 'Interfaces',
          items: [
            { text: 'Terminal UI (TUI)', link: '/features/tui-mode' },
            { text: 'Editor Integration', link: '/features/editor-integration' },
            { text: 'MCP Integration', link: '/features/mcp-integration' }
          ]
        },
        {
          text: 'Search Capabilities',
          items: [
            { text: 'Semantic Search', link: '/features/semantic-search' },
            { text: 'Hybrid Search', link: '/features/hybrid-search' },
            { text: 'grep Compatibility', link: '/features/grep-compatibility' }
          ]
        }
      ],
      '/reference/': [
        {
          text: 'Reference',
          items: [
            { text: 'CLI Reference', link: '/reference/cli' },
            { text: 'Output Formats', link: '/reference/output-formats' },
            { text: 'Embedding Models', link: '/reference/models' },
            { text: 'Configuration', link: '/reference/configuration' },
            { text: 'Advanced Configuration', link: '/reference/advanced' },
            { text: 'Architecture', link: '/reference/architecture' }
          ]
        }
      ],
      '/contributing/': [
        {
          text: 'Contributing',
          items: [
            { text: 'Development Guide', link: '/contributing/development' },
            { text: 'Release Process', link: '/contributing/release-process' },
            { text: 'Testing', link: '/contributing/testing' }
          ]
        }
      ]
    },

    socialLinks: [
      { icon: 'github', link: 'https://github.com/BeaconBay/ck' }
    ],

    footer: {
      message: 'Released under the MIT or Apache-2.0 License.',
      copyright: 'Copyright © 2025 BeaconBay'
    },

    search: {
      provider: 'local'
    },

    editLink: {
      pattern: 'https://github.com/BeaconBay/ck/edit/main/docs-site/:path',
      text: 'Edit this page on GitHub'
    }
  }
})
