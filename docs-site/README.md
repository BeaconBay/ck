# ck Documentation Site

VitePress-powered documentation for the ck hybrid code search tool.

## Prerequisites

- Node.js 18+ or 20+
- pnpm 10+

## Installation

```bash
# Install pnpm if you haven't already
npm install -g pnpm

# Install dependencies
pnpm install
```

## Development

Start the local development server with hot reload:

```bash
pnpm dev
```

The site will be available at `http://localhost:5173` (or next available port).

## Building

Build the documentation site for production:

```bash
pnpm build
```

Output will be in `.vitepress/dist/`.

## Preview

Preview the production build locally:

```bash
pnpm build
pnpm preview
```

## Project Structure

```
docs-site/
├── .vitepress/
│   ├── config.ts           # Site configuration
│   ├── dist/               # Build output (gitignored)
│   └── cache/              # Build cache (gitignored)
├── guide/                  # Getting started guides
│   ├── introduction.md
│   ├── installation.md
│   ├── basic-usage.md
│   └── advanced-usage.md
├── features/               # Feature documentation
│   ├── semantic-search.md
│   ├── mcp-integration.md
│   ├── hybrid-search.md
│   └── grep-compatibility.md
├── reference/              # Reference documentation
│   ├── cli.md
│   ├── models.md
│   ├── configuration.md
│   └── architecture.md
├── contributing/           # Contributing guides
│   ├── development.md
│   ├── release-process.md
│   └── testing.md
├── index.md                # Landing page
├── package.json            # Dependencies and scripts
└── README.md               # This file
```

## Writing Documentation

### Markdown Features

VitePress supports enhanced markdown:

#### Code Blocks with Syntax Highlighting

\`\`\`rust
fn main() {
    println!(“Hello, world!”);
}
\`\`\`

#### Code Groups

\`\`\`rust
// Rust example
\`\`\`

\`\`\`python
# Python example
\`\`\`

#### Custom Containers

\`\`\`markdown
::: tip
This is a tip
:::

::: warning
This is a warning
:::

::: danger
This is a danger notice
:::
\`\`\`

#### Tables

```markdown
| Header 1 | Header 2 |
|----------|----------|
| Cell 1   | Cell 2   |
```

### Adding New Pages

1. Create markdown file in appropriate directory
2. Add to sidebar in `.vitepress/config.ts`:

```typescript
sidebar: {
  '/guide/': [
    {
      text: 'Guide',
      items: [
        { text: 'New Page', link: '/guide/new-page' }
      ]
    }
  ]
}
```

### Internal Links

```markdown
[Link text](/guide/installation)
[Another page](/features/semantic-search#section)
```

### Images

Place images in `public/` directory and reference:

```markdown
![Alt text](/image.png)
```

## Configuration

Main config file: `.vitepress/config.ts`

Key sections:
- `title`: Site title
- `description`: Site description
- `themeConfig.nav`: Top navigation
- `themeConfig.sidebar`: Sidebar navigation
- `themeConfig.search`: Search configuration

## Deployment

### GitHub Pages

Add to `.github/workflows/deploy-docs.yml`:

```yaml
name: Deploy Docs

on:
  push:
    branches: [main]
    paths:
      - 'docs-site/**'

jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: pnpm/action-setup@v2
        with:
          version: 10
      - uses: actions/setup-node@v4
        with:
          node-version: 20
          cache: 'pnpm'
          cache-dependency-path: docs-site/pnpm-lock.yaml
      - run: cd docs-site && pnpm install
      - run: cd docs-site && pnpm build
      - uses: peaceiris/actions-gh-pages@v3
        with:
          github_token: ${{ secrets.GITHUB_TOKEN }}
          publish_dir: docs-site/.vitepress/dist
```

### Netlify

1. Connect repository to Netlify
2. Set build settings:
   — Base directory: `docs-site`
   — Build command: `pnpm build`
   — Publish directory: `docs-site/.vitepress/dist`

### Vercel

```json
{
  "buildCommand": "cd docs-site && pnpm build",
  "outputDirectory": "docs-site/.vitepress/dist"
}
```

## Troubleshooting

### Port Already in Use

```bash
# Kill process on port 5173
lsof -ti:5173 | xargs kill -9

# Or use different port
pnpm dev --port 5174
```

### Build Errors

```bash
# Clear cache and rebuild
rm -rf .vitepress/cache .vitepress/dist node_modules
pnpm install
pnpm build
```

### Broken Links

VitePress checks for broken links during build. Fix any reported issues.

## Contributing to Docs

1. Fork the repository
2. Create feature branch
3. Make changes to docs
4. Test locally with `pnpm dev`
5. Build to verify: `pnpm build`
6. Submit pull request

## Resources

- [VitePress Documentation](https://vitepress.dev/)
- [Markdown Guide](https://vitepress.dev/guide/markdown)
- [Vue.js Documentation](https://vuejs.org/) (for custom components)

## License

MIT or Apache-2.0 (same as ck project)
