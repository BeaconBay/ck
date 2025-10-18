import{_ as a,c as n,o as i,ag as e}from"./chunks/framework.BmMagzGG.js";const d=JSON.parse(`{"title":"Configuration","description":"Configure ck's indexing behavior with .ckignore files, exclusion patterns, and project-specific settings for optimal semantic search.","frontmatter":{"title":"Configuration","description":"Configure ck's indexing behavior with .ckignore files, exclusion patterns, and project-specific settings for optimal semantic search."},"headers":[],"relativePath":"reference/configuration.md","filePath":"reference/configuration.md"}`),p={name:"reference/configuration.md"};function l(t,s,h,c,k,o){return i(),n("div",null,[...s[0]||(s[0]=[e(`<h1 id="configuration" tabindex="-1">Configuration <a class="header-anchor" href="#configuration" aria-label="Permalink to &quot;Configuration&quot;">​</a></h1><p>Configure ck’s indexing and exclusion behavior.</p><h2 id="ckignore-file" tabindex="-1">.ckignore File <a class="header-anchor" href="#ckignore-file" aria-label="Permalink to &quot;.ckignore File&quot;">​</a></h2><p>Control which files are excluded from indexing using <code>.ckignore</code> in your repository root.</p><h3 id="syntax" tabindex="-1">Syntax <a class="header-anchor" href="#syntax" aria-label="Permalink to &quot;Syntax&quot;">​</a></h3><p>Uses gitignore-style patterns:</p><div class="language-txt vp-adaptive-theme"><button title="Copy Code" class="copy"></button><span class="lang">txt</span><pre class="shiki shiki-themes github-light github-dark vp-code" tabindex="0"><code><span class="line"><span># .ckignore syntax</span></span>
<span class="line"><span># Comments start with #</span></span>
<span class="line"><span></span></span>
<span class="line"><span># Exclude directories</span></span>
<span class="line"><span>node_modules/</span></span>
<span class="line"><span>target/</span></span>
<span class="line"><span>dist/</span></span>
<span class="line"><span>.git/</span></span>
<span class="line"><span></span></span>
<span class="line"><span># Exclude file patterns</span></span>
<span class="line"><span>*.log</span></span>
<span class="line"><span>*.tmp</span></span>
<span class="line"><span>*.bak</span></span>
<span class="line"><span></span></span>
<span class="line"><span># Exclude specific files</span></span>
<span class="line"><span>.env</span></span>
<span class="line"><span>config/secrets.yaml</span></span>
<span class="line"><span></span></span>
<span class="line"><span># Negation (include despite parent exclusion)</span></span>
<span class="line"><span>!important.log</span></span>
<span class="line"><span></span></span>
<span class="line"><span># Wildcards</span></span>
<span class="line"><span>temp_*.txt</span></span>
<span class="line"><span>**/*.test.js</span></span></code></pre></div><h3 id="default-exclusions" tabindex="-1">Default Exclusions <a class="header-anchor" href="#default-exclusions" aria-label="Permalink to &quot;Default Exclusions&quot;">​</a></h3><p>Auto-created <code>.ckignore</code> includes:</p><div class="language-txt vp-adaptive-theme"><button title="Copy Code" class="copy"></button><span class="lang">txt</span><pre class="shiki shiki-themes github-light github-dark vp-code" tabindex="0"><code><span class="line"><span># Default .ckignore patterns</span></span>
<span class="line"><span># Images</span></span>
<span class="line"><span>*.png</span></span>
<span class="line"><span>*.jpg</span></span>
<span class="line"><span>*.jpeg</span></span>
<span class="line"><span>*.gif</span></span>
<span class="line"><span>*.svg</span></span>
<span class="line"><span>*.ico</span></span>
<span class="line"><span>*.webp</span></span>
<span class="line"><span></span></span>
<span class="line"><span># Videos</span></span>
<span class="line"><span>*.mp4</span></span>
<span class="line"><span>*.avi</span></span>
<span class="line"><span>*.mov</span></span>
<span class="line"><span>*.mkv</span></span>
<span class="line"><span>*.webm</span></span>
<span class="line"><span></span></span>
<span class="line"><span># Audio</span></span>
<span class="line"><span>*.mp3</span></span>
<span class="line"><span>*.wav</span></span>
<span class="line"><span>*.flac</span></span>
<span class="line"><span>*.aac</span></span>
<span class="line"><span></span></span>
<span class="line"><span># Archives</span></span>
<span class="line"><span>*.zip</span></span>
<span class="line"><span>*.tar</span></span>
<span class="line"><span>*.gz</span></span>
<span class="line"><span>*.rar</span></span>
<span class="line"><span>*.7z</span></span>
<span class="line"><span></span></span>
<span class="line"><span># Binaries</span></span>
<span class="line"><span>*.exe</span></span>
<span class="line"><span>*.dll</span></span>
<span class="line"><span>*.so</span></span>
<span class="line"><span>*.dylib</span></span>
<span class="line"><span></span></span>
<span class="line"><span># Config files</span></span>
<span class="line"><span>*.json</span></span>
<span class="line"><span>*.yaml</span></span>
<span class="line"><span>*.yml</span></span>
<span class="line"><span>*.toml</span></span>
<span class="line"><span></span></span>
<span class="line"><span># Build artifacts</span></span>
<span class="line"><span>target/</span></span>
<span class="line"><span>dist/</span></span>
<span class="line"><span>build/</span></span>
<span class="line"><span>node_modules/</span></span></code></pre></div><h3 id="editing-ckignore" tabindex="-1">Editing .ckignore <a class="header-anchor" href="#editing-ckignore" aria-label="Permalink to &quot;Editing .ckignore&quot;">​</a></h3><div class="language-bash vp-adaptive-theme"><button title="Copy Code" class="copy"></button><span class="lang">bash</span><pre class="shiki shiki-themes github-light github-dark vp-code" tabindex="0"><code><span class="line"><span style="--shiki-light:#6A737D;--shiki-dark:#6A737D;"># Create or edit</span></span>
<span class="line"><span style="--shiki-light:#6F42C1;--shiki-dark:#B392F0;">vim</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> .ckignore</span></span>
<span class="line"></span>
<span class="line"><span style="--shiki-light:#6A737D;--shiki-dark:#6A737D;"># Add patterns</span></span>
<span class="line"><span style="--shiki-light:#005CC5;--shiki-dark:#79B8FF;">echo</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> &quot;logs/&quot;</span><span style="--shiki-light:#D73A49;--shiki-dark:#F97583;"> &gt;&gt;</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> .ckignore</span></span>
<span class="line"><span style="--shiki-light:#005CC5;--shiki-dark:#79B8FF;">echo</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> &quot;*.cache&quot;</span><span style="--shiki-light:#D73A49;--shiki-dark:#F97583;"> &gt;&gt;</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> .ckignore</span></span>
<span class="line"></span>
<span class="line"><span style="--shiki-light:#6A737D;--shiki-dark:#6A737D;"># Rebuild index to apply changes</span></span>
<span class="line"><span style="--shiki-light:#6F42C1;--shiki-dark:#B392F0;">ck</span><span style="--shiki-light:#005CC5;--shiki-dark:#79B8FF;"> --clean</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> .</span></span>
<span class="line"><span style="--shiki-light:#6F42C1;--shiki-dark:#B392F0;">ck</span><span style="--shiki-light:#005CC5;--shiki-dark:#79B8FF;"> --index</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> .</span></span></code></pre></div><h2 id="exclusion-layers" tabindex="-1">Exclusion Layers <a class="header-anchor" href="#exclusion-layers" aria-label="Permalink to &quot;Exclusion Layers&quot;">​</a></h2><p>ck combines multiple exclusion sources (all additive):</p><h3 id="_1-default-exclusions" tabindex="-1">1. Default Exclusions <a class="header-anchor" href="#_1-default-exclusions" aria-label="Permalink to &quot;1. Default Exclusions&quot;">​</a></h3><p>Built-in patterns (binaries, common artifacts).</p><h3 id="_2-gitignore" tabindex="-1">2. .gitignore <a class="header-anchor" href="#_2-gitignore" aria-label="Permalink to &quot;2. .gitignore&quot;">​</a></h3><p>Automatically respected unless <code>--no-ignore</code> is used.</p><h3 id="_3-ckignore" tabindex="-1">3. .ckignore <a class="header-anchor" href="#_3-ckignore" aria-label="Permalink to &quot;3. .ckignore&quot;">​</a></h3><p>Project-specific semantic search exclusions.</p><h3 id="_4-cli-exclusions" tabindex="-1">4. CLI Exclusions <a class="header-anchor" href="#_4-cli-exclusions" aria-label="Permalink to &quot;4. CLI Exclusions&quot;">​</a></h3><p>Command-line <code>--exclude</code> flags.</p><h3 id="examples" tabindex="-1">Examples <a class="header-anchor" href="#examples" aria-label="Permalink to &quot;Examples&quot;">​</a></h3><div class="language-bash vp-adaptive-theme"><button title="Copy Code" class="copy"></button><span class="lang">bash</span><pre class="shiki shiki-themes github-light github-dark vp-code" tabindex="0"><code><span class="line"><span style="--shiki-light:#6A737D;--shiki-dark:#6A737D;"># All layers active (default)</span></span>
<span class="line"><span style="--shiki-light:#6F42C1;--shiki-dark:#B392F0;">ck</span><span style="--shiki-light:#005CC5;--shiki-dark:#79B8FF;"> --sem</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> &quot;pattern&quot;</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> .</span></span>
<span class="line"></span>
<span class="line"><span style="--shiki-light:#6A737D;--shiki-dark:#6A737D;"># Skip .gitignore only</span></span>
<span class="line"><span style="--shiki-light:#6F42C1;--shiki-dark:#B392F0;">ck</span><span style="--shiki-light:#005CC5;--shiki-dark:#79B8FF;"> --no-ignore</span><span style="--shiki-light:#005CC5;--shiki-dark:#79B8FF;"> --sem</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> &quot;pattern&quot;</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> .</span></span>
<span class="line"></span>
<span class="line"><span style="--shiki-light:#6A737D;--shiki-dark:#6A737D;"># Skip .ckignore only</span></span>
<span class="line"><span style="--shiki-light:#6F42C1;--shiki-dark:#B392F0;">ck</span><span style="--shiki-light:#005CC5;--shiki-dark:#79B8FF;"> --no-ckignore</span><span style="--shiki-light:#005CC5;--shiki-dark:#79B8FF;"> --sem</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> &quot;pattern&quot;</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> .</span></span>
<span class="line"></span>
<span class="line"><span style="--shiki-light:#6A737D;--shiki-dark:#6A737D;"># Skip both ignore files</span></span>
<span class="line"><span style="--shiki-light:#6F42C1;--shiki-dark:#B392F0;">ck</span><span style="--shiki-light:#005CC5;--shiki-dark:#79B8FF;"> --no-ignore</span><span style="--shiki-light:#005CC5;--shiki-dark:#79B8FF;"> --no-ckignore</span><span style="--shiki-light:#005CC5;--shiki-dark:#79B8FF;"> --sem</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> &quot;pattern&quot;</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> .</span></span>
<span class="line"></span>
<span class="line"><span style="--shiki-light:#6A737D;--shiki-dark:#6A737D;"># Add CLI exclusions</span></span>
<span class="line"><span style="--shiki-light:#6F42C1;--shiki-dark:#B392F0;">ck</span><span style="--shiki-light:#005CC5;--shiki-dark:#79B8FF;"> --exclude</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> &quot;temp/&quot;</span><span style="--shiki-light:#005CC5;--shiki-dark:#79B8FF;"> --sem</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> &quot;pattern&quot;</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> .</span></span>
<span class="line"></span>
<span class="line"><span style="--shiki-light:#6A737D;--shiki-dark:#6A737D;"># Multiple CLI exclusions</span></span>
<span class="line"><span style="--shiki-light:#6F42C1;--shiki-dark:#B392F0;">ck</span><span style="--shiki-light:#005CC5;--shiki-dark:#79B8FF;"> --exclude</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> &quot;*.test.js&quot;</span><span style="--shiki-light:#005CC5;--shiki-dark:#79B8FF;"> --exclude</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> &quot;fixtures/&quot;</span><span style="--shiki-light:#005CC5;--shiki-dark:#79B8FF;"> --sem</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> &quot;pattern&quot;</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> .</span></span></code></pre></div><h2 id="index-location" tabindex="-1">Index Location <a class="header-anchor" href="#index-location" aria-label="Permalink to &quot;Index Location&quot;">​</a></h2><p>Indexes stored in <code>.ck/</code> directories:</p><div class="language- vp-adaptive-theme"><button title="Copy Code" class="copy"></button><span class="lang"></span><pre class="shiki shiki-themes github-light github-dark vp-code" tabindex="0"><code><span class="line"><span>project/</span></span>
<span class="line"><span>├── src/</span></span>
<span class="line"><span>├── .ck/                    # Index directory (safe to delete)</span></span>
<span class="line"><span>│   ├── embeddings.json     # Embedding vectors</span></span>
<span class="line"><span>│   ├── ann_index.bin       # Vector index</span></span>
<span class="line"><span>│   ├── tantivy_index/      # Keyword search index</span></span>
<span class="line"><span>│   └── manifest.json       # Index metadata</span></span>
<span class="line"><span>├── .ckignore               # Exclusion patterns</span></span>
<span class="line"><span>└── .gitignore</span></span></code></pre></div><h3 id="index-management" tabindex="-1">Index Management <a class="header-anchor" href="#index-management" aria-label="Permalink to &quot;Index Management&quot;">​</a></h3><div class="language-bash vp-adaptive-theme"><button title="Copy Code" class="copy"></button><span class="lang">bash</span><pre class="shiki shiki-themes github-light github-dark vp-code" tabindex="0"><code><span class="line"><span style="--shiki-light:#6A737D;--shiki-dark:#6A737D;"># Check index status</span></span>
<span class="line"><span style="--shiki-light:#6F42C1;--shiki-dark:#B392F0;">ck</span><span style="--shiki-light:#005CC5;--shiki-dark:#79B8FF;"> --status</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> .</span></span>
<span class="line"></span>
<span class="line"><span style="--shiki-light:#6A737D;--shiki-dark:#6A737D;"># View index details</span></span>
<span class="line"><span style="--shiki-light:#6F42C1;--shiki-dark:#B392F0;">ck</span><span style="--shiki-light:#005CC5;--shiki-dark:#79B8FF;"> --status</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> src/</span></span>
<span class="line"></span>
<span class="line"><span style="--shiki-light:#6A737D;--shiki-dark:#6A737D;"># Remove index</span></span>
<span class="line"><span style="--shiki-light:#6F42C1;--shiki-dark:#B392F0;">rm</span><span style="--shiki-light:#005CC5;--shiki-dark:#79B8FF;"> -rf</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> .ck/</span></span>
<span class="line"></span>
<span class="line"><span style="--shiki-light:#6A737D;--shiki-dark:#6A737D;"># Or use clean command</span></span>
<span class="line"><span style="--shiki-light:#6F42C1;--shiki-dark:#B392F0;">ck</span><span style="--shiki-light:#005CC5;--shiki-dark:#79B8FF;"> --clean</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> .</span></span></code></pre></div><h2 id="index-metadata" tabindex="-1">Index Metadata <a class="header-anchor" href="#index-metadata" aria-label="Permalink to &quot;Index Metadata&quot;">​</a></h2><p>Index manifest stores:</p><ul><li>Embedding model used</li><li>Model dimensions</li><li>Creation timestamp</li><li>File hashes for delta indexing</li></ul><p>View with:</p><div class="language-bash vp-adaptive-theme"><button title="Copy Code" class="copy"></button><span class="lang">bash</span><pre class="shiki shiki-themes github-light github-dark vp-code" tabindex="0"><code><span class="line"><span style="--shiki-light:#6F42C1;--shiki-dark:#B392F0;">cat</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> .ck/manifest.json</span></span></code></pre></div><h2 id="configuration-best-practices" tabindex="-1">Configuration Best Practices <a class="header-anchor" href="#configuration-best-practices" aria-label="Permalink to &quot;Configuration Best Practices&quot;">​</a></h2><h3 id="what-to-exclude" tabindex="-1">What to Exclude <a class="header-anchor" href="#what-to-exclude" aria-label="Permalink to &quot;What to Exclude&quot;">​</a></h3><p>✅ <strong>Exclude:</strong></p><ul><li>Generated files (build artifacts)</li><li>Media files (images, videos, audio)</li><li>Large data files</li><li>Config files (JSON, YAML)</li><li>Dependencies (node_modules, vendor)</li><li>Test fixtures</li><li>Documentation (if not relevant to code search)</li></ul><p>❌ <strong>Don’t exclude:</strong></p><ul><li>Source code</li><li>Important configuration (if you search it)</li><li>Tests (if you want to find them)</li><li>Documentation (if you search it)</li></ul><h3 id="example-ckignore-for-different-projects" tabindex="-1">Example .ckignore for Different Projects <a class="header-anchor" href="#example-ckignore-for-different-projects" aria-label="Permalink to &quot;Example .ckignore for Different Projects&quot;">​</a></h3><p><strong>JavaScript/Node.js:</strong></p><div class="language-txt vp-adaptive-theme"><button title="Copy Code" class="copy"></button><span class="lang">txt</span><pre class="shiki shiki-themes github-light github-dark vp-code" tabindex="0"><code><span class="line"><span># .ckignore for JavaScript/Node.js projects</span></span>
<span class="line"><span>node_modules/</span></span>
<span class="line"><span>dist/</span></span>
<span class="line"><span>build/</span></span>
<span class="line"><span>*.json</span></span>
<span class="line"><span>*.yaml</span></span>
<span class="line"><span>*.log</span></span>
<span class="line"><span>.next/</span></span>
<span class="line"><span>.nuxt/</span></span>
<span class="line"><span>coverage/</span></span></code></pre></div><p><strong>Rust:</strong></p><div class="language-txt vp-adaptive-theme"><button title="Copy Code" class="copy"></button><span class="lang">txt</span><pre class="shiki shiki-themes github-light github-dark vp-code" tabindex="0"><code><span class="line"><span># .ckignore for Rust projects</span></span>
<span class="line"><span>target/</span></span>
<span class="line"><span>Cargo.lock</span></span>
<span class="line"><span>*.json</span></span>
<span class="line"><span>*.toml</span></span>
<span class="line"><span>*.log</span></span></code></pre></div><p><strong>Python:</strong></p><div class="language-txt vp-adaptive-theme"><button title="Copy Code" class="copy"></button><span class="lang">txt</span><pre class="shiki shiki-themes github-light github-dark vp-code" tabindex="0"><code><span class="line"><span># .ckignore for Python projects</span></span>
<span class="line"><span>__pycache__/</span></span>
<span class="line"><span>*.pyc</span></span>
<span class="line"><span>venv/</span></span>
<span class="line"><span>.venv/</span></span>
<span class="line"><span>dist/</span></span>
<span class="line"><span>build/</span></span>
<span class="line"><span>*.egg-info/</span></span>
<span class="line"><span>.pytest_cache/</span></span></code></pre></div><p><strong>Go:</strong></p><div class="language-txt vp-adaptive-theme"><button title="Copy Code" class="copy"></button><span class="lang">txt</span><pre class="shiki shiki-themes github-light github-dark vp-code" tabindex="0"><code><span class="line"><span># .ckignore for Go projects</span></span>
<span class="line"><span>vendor/</span></span>
<span class="line"><span>*.mod</span></span>
<span class="line"><span>*.sum</span></span>
<span class="line"><span>bin/</span></span></code></pre></div><h2 id="performance-tuning" tabindex="-1">Performance Tuning <a class="header-anchor" href="#performance-tuning" aria-label="Permalink to &quot;Performance Tuning&quot;">​</a></h2><h3 id="reduce-index-size" tabindex="-1">Reduce Index Size <a class="header-anchor" href="#reduce-index-size" aria-label="Permalink to &quot;Reduce Index Size&quot;">​</a></h3><div class="language-bash vp-adaptive-theme"><button title="Copy Code" class="copy"></button><span class="lang">bash</span><pre class="shiki shiki-themes github-light github-dark vp-code" tabindex="0"><code><span class="line"><span style="--shiki-light:#6A737D;--shiki-dark:#6A737D;"># Exclude documentation</span></span>
<span class="line"><span style="--shiki-light:#005CC5;--shiki-dark:#79B8FF;">echo</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> &quot;docs/&quot;</span><span style="--shiki-light:#D73A49;--shiki-dark:#F97583;"> &gt;&gt;</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> .ckignore</span></span>
<span class="line"><span style="--shiki-light:#005CC5;--shiki-dark:#79B8FF;">echo</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> &quot;*.md&quot;</span><span style="--shiki-light:#D73A49;--shiki-dark:#F97583;"> &gt;&gt;</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> .ckignore</span></span>
<span class="line"></span>
<span class="line"><span style="--shiki-light:#6A737D;--shiki-dark:#6A737D;"># Exclude tests</span></span>
<span class="line"><span style="--shiki-light:#005CC5;--shiki-dark:#79B8FF;">echo</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> &quot;tests/&quot;</span><span style="--shiki-light:#D73A49;--shiki-dark:#F97583;"> &gt;&gt;</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> .ckignore</span></span>
<span class="line"><span style="--shiki-light:#005CC5;--shiki-dark:#79B8FF;">echo</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> &quot;*_test.rs&quot;</span><span style="--shiki-light:#D73A49;--shiki-dark:#F97583;"> &gt;&gt;</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> .ckignore</span></span>
<span class="line"></span>
<span class="line"><span style="--shiki-light:#6A737D;--shiki-dark:#6A737D;"># Rebuild</span></span>
<span class="line"><span style="--shiki-light:#6F42C1;--shiki-dark:#B392F0;">ck</span><span style="--shiki-light:#005CC5;--shiki-dark:#79B8FF;"> --clean</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> .</span></span>
<span class="line"><span style="--shiki-light:#6F42C1;--shiki-dark:#B392F0;">ck</span><span style="--shiki-light:#005CC5;--shiki-dark:#79B8FF;"> --index</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> .</span></span></code></pre></div><h3 id="faster-indexing" tabindex="-1">Faster Indexing <a class="header-anchor" href="#faster-indexing" aria-label="Permalink to &quot;Faster Indexing&quot;">​</a></h3><div class="language-bash vp-adaptive-theme"><button title="Copy Code" class="copy"></button><span class="lang">bash</span><pre class="shiki shiki-themes github-light github-dark vp-code" tabindex="0"><code><span class="line"><span style="--shiki-light:#6A737D;--shiki-dark:#6A737D;"># Exclude large directories first</span></span>
<span class="line"><span style="--shiki-light:#005CC5;--shiki-dark:#79B8FF;">echo</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> &quot;node_modules/&quot;</span><span style="--shiki-light:#D73A49;--shiki-dark:#F97583;"> &gt;</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> .ckignore</span></span>
<span class="line"><span style="--shiki-light:#005CC5;--shiki-dark:#79B8FF;">echo</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> &quot;target/&quot;</span><span style="--shiki-light:#D73A49;--shiki-dark:#F97583;"> &gt;&gt;</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> .ckignore</span></span>
<span class="line"></span>
<span class="line"><span style="--shiki-light:#6A737D;--shiki-dark:#6A737D;"># Use smaller model</span></span>
<span class="line"><span style="--shiki-light:#6F42C1;--shiki-dark:#B392F0;">ck</span><span style="--shiki-light:#005CC5;--shiki-dark:#79B8FF;"> --index</span><span style="--shiki-light:#005CC5;--shiki-dark:#79B8FF;"> --model</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> bge-small</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> .</span></span>
<span class="line"></span>
<span class="line"><span style="--shiki-light:#6A737D;--shiki-dark:#6A737D;"># Index specific paths</span></span>
<span class="line"><span style="--shiki-light:#6F42C1;--shiki-dark:#B392F0;">ck</span><span style="--shiki-light:#005CC5;--shiki-dark:#79B8FF;"> --index</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> src/</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> lib/</span></span></code></pre></div><h2 id="troubleshooting" tabindex="-1">Troubleshooting <a class="header-anchor" href="#troubleshooting" aria-label="Permalink to &quot;Troubleshooting&quot;">​</a></h2><h3 id="files-not-indexed" tabindex="-1">Files Not Indexed <a class="header-anchor" href="#files-not-indexed" aria-label="Permalink to &quot;Files Not Indexed&quot;">​</a></h3><div class="language-bash vp-adaptive-theme"><button title="Copy Code" class="copy"></button><span class="lang">bash</span><pre class="shiki shiki-themes github-light github-dark vp-code" tabindex="0"><code><span class="line"><span style="--shiki-light:#6A737D;--shiki-dark:#6A737D;"># Check if file is excluded</span></span>
<span class="line"><span style="--shiki-light:#6F42C1;--shiki-dark:#B392F0;">cat</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> .ckignore</span></span>
<span class="line"><span style="--shiki-light:#6F42C1;--shiki-dark:#B392F0;">cat</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> .gitignore</span></span>
<span class="line"></span>
<span class="line"><span style="--shiki-light:#6A737D;--shiki-dark:#6A737D;"># Try without ignore files</span></span>
<span class="line"><span style="--shiki-light:#6F42C1;--shiki-dark:#B392F0;">ck</span><span style="--shiki-light:#005CC5;--shiki-dark:#79B8FF;"> --no-ignore</span><span style="--shiki-light:#005CC5;--shiki-dark:#79B8FF;"> --no-ckignore</span><span style="--shiki-light:#005CC5;--shiki-dark:#79B8FF;"> --sem</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> &quot;pattern&quot;</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> .</span></span>
<span class="line"></span>
<span class="line"><span style="--shiki-light:#6A737D;--shiki-dark:#6A737D;"># Check if file is binary</span></span>
<span class="line"><span style="--shiki-light:#6F42C1;--shiki-dark:#B392F0;">file</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> path/to/file</span></span></code></pre></div><h3 id="too-many-files-indexed" tabindex="-1">Too Many Files Indexed <a class="header-anchor" href="#too-many-files-indexed" aria-label="Permalink to &quot;Too Many Files Indexed&quot;">​</a></h3><div class="language-bash vp-adaptive-theme"><button title="Copy Code" class="copy"></button><span class="lang">bash</span><pre class="shiki shiki-themes github-light github-dark vp-code" tabindex="0"><code><span class="line"><span style="--shiki-light:#6A737D;--shiki-dark:#6A737D;"># Check index status</span></span>
<span class="line"><span style="--shiki-light:#6F42C1;--shiki-dark:#B392F0;">ck</span><span style="--shiki-light:#005CC5;--shiki-dark:#79B8FF;"> --status</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> .</span></span>
<span class="line"></span>
<span class="line"><span style="--shiki-light:#6A737D;--shiki-dark:#6A737D;"># Add exclusions to .ckignore</span></span>
<span class="line"><span style="--shiki-light:#6F42C1;--shiki-dark:#B392F0;">vim</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> .ckignore</span></span>
<span class="line"></span>
<span class="line"><span style="--shiki-light:#6A737D;--shiki-dark:#6A737D;"># Rebuild</span></span>
<span class="line"><span style="--shiki-light:#6F42C1;--shiki-dark:#B392F0;">ck</span><span style="--shiki-light:#005CC5;--shiki-dark:#79B8FF;"> --clean</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> .</span></span>
<span class="line"><span style="--shiki-light:#6F42C1;--shiki-dark:#B392F0;">ck</span><span style="--shiki-light:#005CC5;--shiki-dark:#79B8FF;"> --index</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> .</span></span></code></pre></div><h3 id="index-too-large" tabindex="-1">Index Too Large <a class="header-anchor" href="#index-too-large" aria-label="Permalink to &quot;Index Too Large&quot;">​</a></h3><div class="language-bash vp-adaptive-theme"><button title="Copy Code" class="copy"></button><span class="lang">bash</span><pre class="shiki shiki-themes github-light github-dark vp-code" tabindex="0"><code><span class="line"><span style="--shiki-light:#6A737D;--shiki-dark:#6A737D;"># Check .ck/ directory size</span></span>
<span class="line"><span style="--shiki-light:#6F42C1;--shiki-dark:#B392F0;">du</span><span style="--shiki-light:#005CC5;--shiki-dark:#79B8FF;"> -sh</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> .ck/</span></span>
<span class="line"></span>
<span class="line"><span style="--shiki-light:#6A737D;--shiki-dark:#6A737D;"># Exclude unnecessary files</span></span>
<span class="line"><span style="--shiki-light:#005CC5;--shiki-dark:#79B8FF;">echo</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> &quot;docs/&quot;</span><span style="--shiki-light:#D73A49;--shiki-dark:#F97583;"> &gt;&gt;</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> .ckignore</span></span>
<span class="line"><span style="--shiki-light:#005CC5;--shiki-dark:#79B8FF;">echo</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> &quot;*.md&quot;</span><span style="--shiki-light:#D73A49;--shiki-dark:#F97583;"> &gt;&gt;</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> .ckignore</span></span>
<span class="line"></span>
<span class="line"><span style="--shiki-light:#6A737D;--shiki-dark:#6A737D;"># Use smaller embedding model</span></span>
<span class="line"><span style="--shiki-light:#6F42C1;--shiki-dark:#B392F0;">ck</span><span style="--shiki-light:#005CC5;--shiki-dark:#79B8FF;"> --switch-model</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> bge-small</span><span style="--shiki-light:#032F62;--shiki-dark:#9ECBFF;"> .</span></span></code></pre></div><h2 id="next-steps" tabindex="-1">Next Steps <a class="header-anchor" href="#next-steps" aria-label="Permalink to &quot;Next Steps&quot;">​</a></h2><ul><li>Learn about <a href="/ck/reference/models.html">embedding models</a></li><li>Check <a href="/ck/reference/cli.html">CLI reference</a></li><li>See <a href="/ck/guide/basic-usage.html">basic usage</a></li><li>Explore <a href="/ck/features/semantic-search.html">semantic search</a></li></ul>`,63)])])}const g=a(p,[["render",l]]);export{d as __pageData,g as default};
