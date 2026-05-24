/**
 * Webview script for ck search panel - Crisp UI
 * Handles UI interactions and communication with extension
 */

(function () {
  const vscode = acquireVsCodeApi();

  // State
  let currentResults = [];
  let selectedIndex = -1;
  let config = null;
  let searchTimeout = null;
  let currentMode = 'semantic';
  let currentQuery = '';
  let currentQueryTokens = [];

  // DOM elements
  const searchInput = document.getElementById('searchInput');
  const modeSelector = document.getElementById('modeSelector');
  const reindexButton = document.getElementById('reindexButton');
  const includeInput = document.getElementById('includeInput');
  const excludeInput = document.getElementById('excludeInput');
  const indexStatus = document.getElementById('indexStatus');
  const indexProgress = document.getElementById('indexProgress');
  const indexProgressText = indexProgress ? indexProgress.querySelector('.progress-text') : null;
  const ckignoreButton = document.getElementById('ckignoreButton');
  const refreshStatusButton = document.getElementById('refreshStatusButton');
  const loadingIndicator = document.getElementById('loadingIndicator');
  const resultsContainer = document.getElementById('resultsContainer');
  const errorContainer = document.getElementById('errorContainer');
  const resultCount = document.getElementById('resultCount');
  let hideProgressTimeout = null;

  // Event listeners
  searchInput.addEventListener('input', handleSearchInput);
  searchInput.addEventListener('keydown', handleSearchKeydown);
  modeSelector.addEventListener('change', handleModeChange);
  reindexButton.addEventListener('click', handleReindexClick);
  if (ckignoreButton) {
    ckignoreButton.addEventListener('click', () => {
      vscode.postMessage({ type: 'openCkignore' });
    });
  }
  if (refreshStatusButton) {
    refreshStatusButton.addEventListener('click', () => {
      vscode.postMessage({ type: 'getIndexStatus' });
    });
  }

  // Trigger search when include/exclude patterns change
  if (includeInput) {
    includeInput.addEventListener('input', () => {
      const query = searchInput.value.trim();
      if (query) {
        clearTimeout(searchTimeout);
        searchTimeout = setTimeout(() => {
          performSearch(query, modeSelector.value);
        }, 300);
      }
    });
  }

  if (excludeInput) {
    excludeInput.addEventListener('input', () => {
      const query = searchInput.value.trim();
      if (query) {
        clearTimeout(searchTimeout);
        searchTimeout = setTimeout(() => {
          performSearch(query, modeSelector.value);
        }, 300);
      }
    });
  }

  // Handle messages from extension
  window.addEventListener('message', (event) => {
    const message = event.data;

    switch (message.type) {
      case 'config':
        handleConfig(message.config);
        break;
      case 'searchStarted':
        handleSearchStarted(message.query, message.mode);
        break;
      case 'searchResults':
        handleSearchResults(message);
        break;
      case 'searchError':
        handleSearchError(message.error);
      break;
    case 'indexStatus':
      handleIndexStatus(message.status);
      break;
    case 'indexProgress':
      handleIndexProgress(message.update);
      break;
    }
  });

  // Request initial index status
  vscode.postMessage({ type: 'getIndexStatus' });

  /**
   * Handle search input with debouncing
   */
  function handleSearchInput() {
    clearTimeout(searchTimeout);

    const query = searchInput.value.trim();
    if (!query) {
      showEmptyState();
      return;
    }

    // Debounce search (300ms like TUI)
    searchTimeout = setTimeout(() => {
      performSearch(query, modeSelector.value);
    }, 300);
  }

  /**
   * Handle keyboard navigation in search input
   */
  function handleSearchKeydown(e) {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      selectNext();
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      selectPrevious();
    } else if (e.key === 'Enter') {
      e.preventDefault();
      if (selectedIndex >= 0 && selectedIndex < currentResults.length) {
        openResult(currentResults[selectedIndex]);
      } else {
        performSearch(searchInput.value.trim(), modeSelector.value);
      }
    } else if (e.key === 'Escape') {
      selectedIndex = -1;
      updateSelection();
      searchInput.focus();
    }
  }

  /**
   * Handle mode selector change
   */
  function handleModeChange() {
    currentMode = modeSelector.value;
    const query = searchInput.value.trim();
    if (query) {
      performSearch(query, currentMode);
    }
  }

  /**
   * Handle reindex button click
   */
  function handleReindexClick() {
    vscode.postMessage({ type: 'reindex' });
  }

  /**
   * Perform search
   */
  function performSearch(query, mode) {
    currentMode = mode;
    currentQuery = query;
    currentQueryTokens = buildQueryTokens(query);

    // Parse include/exclude patterns (comma or space separated)
    const includePatterns = includeInput && includeInput.value.trim()
      ? includeInput.value.split(/[,\s]+/).map(p => p.trim()).filter(p => p)
      : [];

    const excludePatterns = excludeInput && excludeInput.value.trim()
      ? excludeInput.value.split(/[,\s]+/).map(p => p.trim()).filter(p => p)
      : [];

    vscode.postMessage({
      type: 'search',
      query,
      mode,
      includePatterns: includePatterns.length > 0 ? includePatterns : undefined,
      excludePatterns: excludePatterns.length > 0 ? excludePatterns : undefined
    });
  }

  /**
   * Handle config from extension
   */
  function handleConfig(newConfig) {
    config = newConfig;
    currentMode = config.defaultMode;
    modeSelector.value = currentMode;
  }

  /**
   * Handle search started
   */
  function handleSearchStarted(query, mode) {
    currentMode = mode;
    loadingIndicator.classList.remove('hidden');
    errorContainer.classList.add('hidden');
    resultsContainer.innerHTML = '';
    currentResults = [];
    selectedIndex = -1;
  }

  /**
   * Handle search results
   */
  function handleSearchResults(message) {
    loadingIndicator.classList.add('hidden');
    currentResults = message.results || [];

    if (currentResults.length === 0) {
      showNoResults();
    } else {
      renderResults(currentResults);
    }

    updateResultCount(message.count, message.totalCount, message.hasMore);
  }

  /**
   * Handle search error
   */
  function handleSearchError(error) {
    loadingIndicator.classList.add('hidden');
    errorContainer.classList.remove('hidden');
    errorContainer.textContent = `Error: ${error}`;
    currentResults = [];
  }

  /**
   * Handle index status
   */
  function handleIndexStatus(status) {
    if (!indexStatus) {
      return;
    }

    const indicator = indexStatus.querySelector('.status-indicator');
    const text = indexStatus.querySelector('.status-text');

    if (!status) {
      indicator?.classList.remove('healthy', 'warning', 'error');
      if (text) {
        text.textContent = 'Index status unavailable';
      }
      return;
    }

    indexStatus.dataset.path = status.path || '';
    indexStatus.title = 'ck respects .gitignore, .ckignore, include/exclude filters, and default VS Code excludes';

    const parts = [];
    const fileCount = typeof status.totalFiles === 'number'
      ? status.totalFiles
      : typeof status.estimatedFileCount === 'number'
        ? status.estimatedFileCount
        : undefined;

    if (status.exists) {
      indicator?.classList.add('healthy');
      indicator?.classList.remove('warning', 'error');

      if (typeof fileCount === 'number') {
        const approxPrefix = status.totalFiles === undefined && status.estimatedFileCount !== undefined ? '≈' : '';
        parts.push(`${approxPrefix}${fileCount.toLocaleString()} ${fileCount === 1 ? 'file' : 'files'}`);
      }

      if (typeof status.totalChunks === 'number') {
        parts.push(`${status.totalChunks.toLocaleString()} chunks`);
      }

      if (typeof status.indexSizeBytes === 'number') {
        parts.push(formatBytes(status.indexSizeBytes));
      }

      if (typeof status.lastModified === 'number' && status.lastModified > 0) {
        parts.push(`updated ${formatRelativeTime(status.lastModified)}`);
      }

      if (parts.length === 0) {
        parts.push('Ready');
      }
    } else {
      indicator?.classList.add('warning');
      indicator?.classList.remove('healthy', 'error');
      parts.push('Not indexed');
      if (typeof fileCount === 'number') {
        parts.push(`~${fileCount.toLocaleString()} files in scope`);
      }
    }

    if (text) {
      text.textContent = parts.join(' · ');
    }
  }

  function handleIndexProgress(update) {
    if (!indexProgress || !indexProgressText || !update) {
      return;
    }

    const message = typeof update.message === 'string' ? update.message.trim() : '';
    if (!message) {
      return;
    }

    clearTimeout(hideProgressTimeout);

    let displayMessage = message;
    if (typeof update.total === 'number' && typeof update.progress === 'number' && update.total > 0) {
      const current = Math.min(update.progress, update.total);
      displayMessage = `${message} (${current}/${update.total})`;
    }

    indexProgress.classList.remove('hidden');
    indexProgress.dataset.source = update.source || '';
    indexProgressText.textContent = displayMessage;

    const lowered = message.toLowerCase();
    const completed = ['complete', 'completed', 'success', 'done', 'indexed'].some((token) => lowered.includes(token));

    const hideDelay = completed ? 3000 : 9000;
    hideProgressTimeout = setTimeout(() => {
      indexProgress.classList.add('hidden');
    }, hideDelay);

    if (completed) {
      setTimeout(() => {
        vscode.postMessage({ type: 'getIndexStatus' });
      }, 500);
    }
  }

  /**
   * Render search results grouped by file with enhanced formatting
   */
  function renderResults(results) {
    resultsContainer.innerHTML = '';

    const partitions = partitionResultsByThreshold(results);
    if (partitions.primary.length === 0 && partitions.secondary.length === 0) {
      showNoResults();
      return;
    }

    const toolbar = createResultsToolbar(partitions.secondary.length > 0 && partitions.threshold > 0);
    resultsContainer.appendChild(toolbar);

    const primaryWrapper = document.createElement('div');
    primaryWrapper.className = 'file-group-stack';
    resultsContainer.appendChild(primaryWrapper);

    const primaryGroups = groupResultsByFile(partitions.primary);
    primaryGroups.forEach((items, filePath) => {
      primaryWrapper.appendChild(createFileGroup(filePath, items, false, 'primary'));
    });

    if (partitions.secondary.length > 0 && partitions.threshold > 0) {
      const nearHeader = document.createElement('div');
      nearHeader.className = 'near-matches-header';
      nearHeader.textContent = `Near matches (score < ${partitions.threshold.toFixed(2)})`;
      resultsContainer.appendChild(nearHeader);

      const secondaryWrapper = document.createElement('div');
      secondaryWrapper.className = 'file-group-stack near-matches';
      resultsContainer.appendChild(secondaryWrapper);

      const secondaryGroups = groupResultsByFile(partitions.secondary);
      secondaryGroups.forEach((items, filePath) => {
        secondaryWrapper.appendChild(createFileGroup(filePath, items, true, 'secondary'));
      });
    }
  }

  /**
   * Build a collapsible group of results for a single file
   */
  function createFileGroup(filePath, items, collapsed = false, variant = 'primary') {
    const group = document.createElement('div');
    group.className = 'file-group';
    if (variant === 'secondary') {
      group.classList.add('file-group--secondary');
    }

    const header = document.createElement('div');
    header.className = 'file-group-header';

    const toggleIcon = document.createElement('span');
    toggleIcon.className = 'toggle-icon';
    toggleIcon.textContent = '▾';

    const nameWrapper = document.createElement('div');
    nameWrapper.className = 'file-group-title';

    const normalizedPath = filePath.replace(/\\/g, '/');
    const baseName = getBasename(normalizedPath);
    const parentPath = normalizedPath.slice(0, normalizedPath.length - baseName.length).replace(/\/$/, '');

    const baseNameSpan = document.createElement('span');
    baseNameSpan.className = 'file-group-basename';
    baseNameSpan.textContent = baseName;
    nameWrapper.appendChild(baseNameSpan);

    if (parentPath) {
      const parentSpan = document.createElement('span');
      parentSpan.className = 'file-group-subpath';
      parentSpan.textContent = ` · ${parentPath}`;
      nameWrapper.appendChild(parentSpan);
    }

    nameWrapper.title = items[0].result.absolutePath || filePath;

    const metricsWrapper = document.createElement('div');
    metricsWrapper.className = 'file-group-metrics';

    const matchCountPill = document.createElement('span');
    matchCountPill.className = 'metric-pill metric-pill--count';
    matchCountPill.textContent = items.length.toString();
    metricsWrapper.appendChild(matchCountPill);

    const bestScore = items
      .map(({ result }) => (typeof result.score === 'number' ? result.score : undefined))
      .filter((score) => score !== undefined);
    if (bestScore.length > 0) {
      const topScore = Math.max(...bestScore);
      const scorePill = document.createElement('span');
      scorePill.className = `metric-pill metric-pill--score ${getScoreLabelClass(topScore)}`;
      scorePill.textContent = topScore.toFixed(topScore >= 0.1 ? 2 : 3);
      metricsWrapper.appendChild(scorePill);
    }

    header.appendChild(toggleIcon);
    header.appendChild(nameWrapper);
    header.appendChild(metricsWrapper);

    const resultsDiv = document.createElement('div');
    resultsDiv.className = 'file-group-results';

    items.forEach(({ result, index }) => {
      const item = createResultItem(result, index, true, variant);
      resultsDiv.appendChild(item);
    });

    header.addEventListener('click', (event) => {
      event.preventDefault();
      const shouldCollapse = !group.classList.contains('collapsed');
      setGroupCollapsed(group, shouldCollapse);
    });

    header.addEventListener('dblclick', (event) => {
      event.preventDefault();
      event.stopPropagation();
      setGroupCollapsed(group, false);
      if (items.length > 0) {
        openResult(items[0].result);
      }
    });

    group.appendChild(header);
    group.appendChild(resultsDiv);
    setGroupCollapsed(group, collapsed);

    return group;
  }

  /**
   * Create a result item element with improved snippet formatting
   */
  function createResultItem(result, index, inGroup = false, variant = 'primary') {
    const item = document.createElement('div');
    item.className = 'result-item';
    if (variant === 'secondary') {
      item.classList.add('result-item--secondary');
    }
    item.dataset.index = index;

    const header = document.createElement('div');
    header.className = 'result-header';

    if (!inGroup) {
      const file = document.createElement('div');
      file.className = 'result-file';
      file.textContent = result.file;
      file.title = result.absolutePath || result.file;
      header.appendChild(file);
    }

    const location = document.createElement('div');
    location.className = 'result-location';

    const lineStart = result.lineStart ?? 0;
    const lineEnd = result.lineEnd ?? result.lineStart;
    const rangeLabel = lineEnd && lineEnd !== lineStart ? `L${lineStart}–${lineEnd}` : `L${lineStart}`;

    const lineNum = document.createElement('span');
    lineNum.className = 'result-line';
    lineNum.textContent = rangeLabel;
    location.appendChild(lineNum);

    if (typeof result.score === 'number') {
      const scoreLabel = document.createElement('span');
      scoreLabel.className = `result-score-label ${getScoreLabelClass(result.score)}`;
      scoreLabel.textContent = `(${result.score.toFixed(2)})`;
      location.appendChild(scoreLabel);
    }

    header.appendChild(location);
    item.appendChild(header);

    const preview = createPreview(result);
    item.appendChild(preview);

    item.addEventListener('click', () => {
      openResult(result);
    });

    return item;
  }

  /**
   * Create preview section with syntax-aware highlighting
   */
  function createPreview(result) {
    const preview = document.createElement('div');
    preview.className = 'result-preview';

    const previewText = result.preview || '';
    const lines = previewText.split('\n');
    const startLine = result.lineStart ?? 0;
    const language = mapLanguage(result.language, result.file);
    const tokens = currentQueryTokens;

    lines.forEach((line, idx) => {
      const lineDiv = document.createElement('div');
      lineDiv.className = 'preview-line';

      const lineNumberValue = startLine + idx;
      lineDiv.addEventListener('click', (event) => {
        event.stopPropagation();
        openResultAtLine(result, lineNumberValue);
      });

      const lineNum = document.createElement('span');
      lineNum.className = 'line-number';
      lineNum.textContent = lineNumberValue.toString();
      lineDiv.appendChild(lineNum);

      const content = document.createElement('span');
      content.className = 'line-content';
      renderLineContent(content, line, language, tokens);
      lineDiv.appendChild(content);

      if (content.querySelector('.match-highlight')) {
        lineDiv.classList.add('match-line');
      }

      preview.appendChild(lineDiv);
    });

    return preview;
  }

  function createResultsToolbar(hasSecondaryGroups) {
    const toolbar = document.createElement('div');
    toolbar.className = 'results-toolbar';

    const label = document.createElement('span');
    label.className = 'toolbar-label';
    label.textContent = 'Search results';

    const buttons = document.createElement('div');
    buttons.className = 'toolbar-buttons';

    const expandBtn = document.createElement('button');
    expandBtn.className = 'toolbar-button';
    expandBtn.textContent = 'Expand all';
    expandBtn.addEventListener('click', () => setAllGroupsCollapsed(false));

    const collapseBtn = document.createElement('button');
    collapseBtn.className = 'toolbar-button';
    collapseBtn.textContent = 'Collapse all';
    collapseBtn.addEventListener('click', () => setAllGroupsCollapsed(true));

    buttons.appendChild(expandBtn);
    buttons.appendChild(collapseBtn);

    if (hasSecondaryGroups) {
      const collapseWeakBtn = document.createElement('button');
      collapseWeakBtn.className = 'toolbar-button';
      collapseWeakBtn.textContent = 'Collapse weak';
      collapseWeakBtn.addEventListener('click', () => {
        resultsContainer.querySelectorAll('.file-group--secondary').forEach((group) => {
          setGroupCollapsed(group, true);
        });
      });
      buttons.appendChild(collapseWeakBtn);
    }

    toolbar.appendChild(label);
    toolbar.appendChild(buttons);

    return toolbar;
  }

  function setAllGroupsCollapsed(collapsed) {
    resultsContainer.querySelectorAll('.file-group').forEach((group) => {
      setGroupCollapsed(group, collapsed);
    });
  }

  function setGroupCollapsed(group, collapsed) {
    if (collapsed) {
      group.classList.add('collapsed');
    } else {
      group.classList.remove('collapsed');
    }

    const icon = group.querySelector('.file-group-header .toggle-icon');
    if (icon) {
      icon.textContent = collapsed ? '▸' : '▾';
    }
  }

  function partitionResultsByThreshold(results) {
    const threshold = typeof config?.threshold === 'number' ? config.threshold : 0;
    const primary = [];
    const secondary = [];

    const hasScores = results.some((result) => typeof result.score === 'number');

    results.forEach((result, index) => {
      const entry = { result, index };
      if (hasScores && threshold > 0 && typeof result.score === 'number' && result.score < threshold) {
        secondary.push(entry);
      } else {
        primary.push(entry);
      }
    });

    return {
      primary,
      secondary,
      threshold: hasScores ? threshold : 0,
    };
  }

  function groupResultsByFile(entries) {
    const groups = new Map();
    entries.forEach((entry) => {
      const key = entry.result.file;
      if (!groups.has(key)) {
        groups.set(key, []);
      }
      groups.get(key).push(entry);
    });
    groups.forEach((groupEntries) => {
      groupEntries.sort((a, b) => {
        const lineA = a.result.lineStart ?? 0;
        const lineB = b.result.lineStart ?? 0;
        return lineA - lineB;
      });
    });
    return groups;
  }
  function getScoreLabelClass(score) {
    if (score >= 0.85) return 'score-label-excellent';
    if (score >= 0.65) return 'score-label-good';
    if (score >= 0.45) return 'score-label-fair';
    return 'score-label-poor';
  }

  function buildQueryTokens(query) {
    if (!query) {
      return [];
    }
    return Array.from(
      new Set(
        query
          .split(/[,\s]+/)
          .map((token) => token.trim())
          .filter((token) => token.length >= 2)
      )
    );
  }

  function mapLanguage(language, filePath) {
    const lang = (language || '').toLowerCase();
    if (lang) {
      return normalizeLanguageAlias(lang);
    }

    if (filePath) {
      const lower = filePath.toLowerCase();
      const extMatch = lower.match(/\.([a-z0-9]+)$/);
      if (extMatch) {
        return normalizeLanguageAlias(extMatch[1]);
      }
    }

    return 'plain';
  }

  function normalizeLanguageAlias(alias) {
    switch (alias) {
      case 'ts':
      case 'tsx':
      case 'typescript':
        return 'typescript';
      case 'js':
      case 'jsx':
      case 'javascript':
        return 'javascript';
      case 'py':
      case 'python':
        return 'python';
      case 'rs':
      case 'rust':
        return 'rust';
      case 'go':
      case 'golang':
        return 'go';
      case 'java':
        return 'java';
      case 'cs':
      case 'csharp':
        return 'csharp';
      case 'kt':
      case 'kotlin':
        return 'kotlin';
      case 'rb':
      case 'ruby':
        return 'ruby';
      case 'php':
        return 'php';
      case 'sh':
      case 'bash':
      case 'shell':
        return 'shell';
      case 'json':
        return 'json';
      case 'yaml':
      case 'yml':
        return 'yaml';
      default:
        return 'plain';
    }
  }

  function renderLineContent(container, line, language, tokens) {
    const escaped = escapeHtml(line);
    const syntaxHighlighted = applySyntaxHighlight(escaped, language);
    const temp = document.createElement('span');
    temp.innerHTML = syntaxHighlighted || '&nbsp;';
    if (tokens.length > 0) {
      applyTokenHighlights(temp, tokens);
    }
    container.innerHTML = temp.innerHTML || '&nbsp;';
  }

  function applySyntaxHighlight(html, language) {
    switch (language) {
      case 'javascript':
      case 'typescript':
      case 'java':
      case 'csharp':
      case 'kotlin':
      case 'php':
        return highlightCStyle(html);
      case 'rust':
        return highlightRust(html);
      case 'python':
        return highlightPython(html);
      case 'go':
        return highlightGo(html);
      case 'ruby':
        return highlightRuby(html);
      case 'shell':
        return highlightShell(html);
      case 'json':
        return highlightJson(html);
      case 'yaml':
        return highlightYaml(html);
      default:
        return html;
    }
  }

  function highlightCStyle(html) {
    html = highlightPattern(html, /(\/{2}[^<]*)$/m, 'comment');
    html = highlightPattern(html, /(\/\*[^*]*\*\/)/g, 'comment');
    html = highlightPattern(html, /(["'`])(?:\\.|(?!\1).)*\1/g, 'string');
    html = highlightPattern(html, /\b(0x[0-9a-fA-F]+|\d+(?:\.\d+)?)\b/g, 'number');
    html = highlightKeywords(html, [
      'const', 'let', 'var', 'function', 'return', 'if', 'else', 'for', 'while',
      'switch', 'case', 'break', 'continue', 'try', 'catch', 'finally', 'throw',
      'import', 'from', 'export', 'extends', 'implements', 'new', 'class',
      'interface', 'enum', 'public', 'private', 'protected', 'static', 'async',
      'await', 'yield', 'package', 'namespace', 'using'
    ]);
    html = highlightFunctionCalls(html);
    return html;
  }

  function highlightRust(html) {
    html = highlightPattern(html, /(\/\/[^<]*)$/m, 'comment');
    html = highlightPattern(html, /(["'])(?:\\.|(?!\1).)*\1/g, 'string');
    html = highlightPattern(html, /\b\d+(?:_\d+)*(?:\.\d+(?:_\d+)*)?\b/g, 'number');
    html = highlightKeywords(html, [
      'fn', 'let', 'mut', 'pub', 'struct', 'enum', 'impl', 'trait', 'match',
      'if', 'else', 'while', 'loop', 'for', 'use', 'mod', 'const', 'static',
      'ref', 'crate', 'super', 'Self', 'self', 'return', 'async', 'await'
    ]);
    html = highlightFunctionCalls(html);
    return html;
  }

  function highlightPython(html) {
    html = highlightPattern(html, /(#.*)$/m, 'comment');
    html = highlightPattern(html, /("""[\s\S]*?"""|'''[\s\S]*?'''|"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*')/g, 'string');
    html = highlightPattern(html, /\b\d+(?:\.\d+)?\b/g, 'number');
    html = highlightKeywords(html, [
      'def', 'return', 'if', 'elif', 'else', 'for', 'while', 'import', 'from',
      'class', 'try', 'except', 'with', 'as', 'lambda', 'yield', 'pass', 'break',
      'continue', 'async', 'await', 'raise', 'True', 'False', 'None'
    ]);
    html = highlightFunctionCalls(html);
    return html;
  }

  function highlightGo(html) {
    html = highlightPattern(html, /(\/\/[^<]*)$/m, 'comment');
    html = highlightPattern(html, /(["'])(?:\\.|(?!\1).)*\1/g, 'string');
    html = highlightPattern(html, /\b\d+(?:\.\d+)?\b/g, 'number');
    html = highlightKeywords(html, [
      'func', 'var', 'const', 'return', 'if', 'else', 'for', 'range', 'import',
      'package', 'type', 'struct', 'interface', 'switch', 'case', 'default',
      'go', 'defer', 'map'
    ]);
    html = highlightFunctionCalls(html);
    return html;
  }

  function highlightRuby(html) {
    html = highlightPattern(html, /(#.*)$/m, 'comment');
    html = highlightPattern(html, /("(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*')/g, 'string');
    html = highlightPattern(html, /\b\d+(?:\.\d+)?\b/g, 'number');
    html = highlightKeywords(html, [
      'def', 'return', 'if', 'elsif', 'else', 'end', 'do', 'while', 'until',
      'class', 'module', 'include', 'extend', 'begin', 'rescue', 'ensure',
      'raise', 'yield', 'self', 'true', 'false', 'nil'
    ]);
    html = highlightFunctionCalls(html);
    return html;
  }

  function highlightShell(html) {
    html = highlightPattern(html, /(#.*)$/m, 'comment');
    html = highlightPattern(html, /("(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*')/g, 'string');
    html = highlightKeywords(html, [
      'if', 'then', 'else', 'elif', 'fi', 'for', 'while', 'do', 'done', 'in',
      'case', 'esac', 'function', 'return'
    ]);
    return html;
  }

  function highlightJson(html) {
    html = highlightPattern(html, /("(?:\\.|[^"\\])*")(?=\s*:)/g, 'keyword');
    html = highlightPattern(html, /("(?:\\.|[^"\\])*")/g, 'string');
    html = highlightPattern(html, /\b(true|false|null)\b/g, 'keyword');
    html = highlightPattern(html, /\b-?\d+(?:\.\d+)?\b/g, 'number');
    return html;
  }

  function highlightYaml(html) {
    html = highlightPattern(html, /(#.*)$/m, 'comment');
    html = highlightPattern(html, /("(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*')/g, 'string');
    html = highlightPattern(html, /^(\s*-)/gm, 'operator');
    html = highlightPattern(html, /(^|\s)([A-Za-z_\-][A-Za-z0-9_\-]*)(?=\s*:)/gm, 'keyword');
    return html;
  }

  function highlightPattern(html, regex, tokenClass) {
    return html.replace(regex, (match) => '<span class="token ' + tokenClass + '">' + match + '</span>');
  }

  function highlightKeywords(html, keywords) {
    const pattern = new RegExp('\\b(' + keywords.map(escapeRegExp).join('|') + ')\\b', 'g');
    return html.replace(pattern, '<span class="token keyword">$1</span>');
  }

  function highlightFunctionCalls(html) {
    return html.replace(/\b([A-Za-z_][A-Za-z0-9_]*)\s*(?=\()/g, '<span class="token function">$1</span>');
  }

  function applyTokenHighlights(root, tokens) {
    if (tokens.length === 0) {
      return;
    }
    const pattern = new RegExp('(' + tokens.map(escapeRegExp).join('|') + ')', 'gi');
    const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
    const nodes = [];
    while (walker.nextNode()) {
      nodes.push(walker.currentNode);
    }
    nodes.forEach((node) => {
      const text = node.nodeValue;
      if (!text) {
        return;
      }
      pattern.lastIndex = 0;
      if (!pattern.test(text)) {
        return;
      }
      pattern.lastIndex = 0;
      const frag = document.createDocumentFragment();
      let lastIndex = 0;
      let match;
      while ((match = pattern.exec(text)) !== null) {
        const start = match.index;
        const end = start + match[0].length;
        if (start > lastIndex) {
          frag.appendChild(document.createTextNode(text.slice(lastIndex, start)));
        }
        const mark = document.createElement('mark');
        mark.className = 'match-highlight';
        mark.textContent = match[0];
        frag.appendChild(mark);
        lastIndex = end;
      }
      if (lastIndex < text.length) {
        frag.appendChild(document.createTextNode(text.slice(lastIndex)));
      }
      node.parentNode.replaceChild(frag, node);
    });
  }

  function escapeHtml(text) {
    return text
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;')
      .replace(/'/g, '&#39;');
  }

  function escapeRegExp(text) {
    return text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  }

  /**
   * Get basename from path
   */
  function getBasename(filePath) {
    const parts = filePath.split(/[/\\]/);
    return parts[parts.length - 1];
  }

  /**
   * Open a result in the editor
   */
  function openResult(result) {
    openResultAtLine(result, result.lineStart);
  }

  function openResultAtLine(result, lineNumber) {
    vscode.postMessage({
      type: 'openFile',
      file: result.absolutePath || result.file,
      line: Math.max(1, lineNumber || 1)
    });
  }

  /**
   * Navigate to next result
   */
  function selectNext() {
    if (currentResults.length === 0) return;

    selectedIndex = (selectedIndex + 1) % currentResults.length;
    updateSelection();
  }

  /**
   * Navigate to previous result
   */
  function selectPrevious() {
    if (currentResults.length === 0) return;

    selectedIndex = selectedIndex <= 0 ? currentResults.length - 1 : selectedIndex - 1;
    updateSelection();
  }

  /**
   * Update visual selection
   */
  function updateSelection() {
    const items = resultsContainer.querySelectorAll('.result-item');

    items.forEach((item, index) => {
      if (index === selectedIndex) {
        item.classList.add('selected');
        item.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
      } else {
        item.classList.remove('selected');
      }
    });
  }

  /**
   * Show empty state
   */
  function showEmptyState() {
    resultsContainer.innerHTML = `
      <div class="empty-state">
        <p>Enter a search query to find code</p>
        <p class="hint">Semantic search understands meaning, not just keywords</p>
      </div>
    `;
    currentResults = [];
    resultCount.innerHTML = '';
  }

  /**
   * Show no results state
   */
  function showNoResults() {
    resultsContainer.innerHTML = `
      <div class="empty-state">
        <p>No results found</p>
        <p class="hint">Try a different query or search mode</p>
      </div>
    `;
    resultCount.innerHTML = '<span class="count-text">0 results</span>';
  }

  /**
   * Update result count display with rerank badge.
   *
   * Builds children via createElement + textContent rather than innerHTML
   * interpolation: CodeQL otherwise flags this as XSS-able since the inputs
   * flow from JSON-RPC, even though they're typed numeric/boolean in
   * practice. DOM-construction makes the safety obvious to both reviewers
   * and the scanner.
   */
  function updateResultCount(count, totalCount, hasMore) {
    let text = `${count} result${count !== 1 ? 's' : ''}`;
    if (totalCount && totalCount !== count) {
      text += ` of ${totalCount}`;
    }
    if (hasMore) {
      text += '+';
    }

    resultCount.textContent = '';

    const countText = document.createElement('span');
    countText.className = 'count-text';
    countText.textContent = text;
    resultCount.appendChild(countText);

    if (currentMode === 'semantic' || currentMode === 'hybrid') {
      const badge = document.createElement('span');
      badge.className = 'rerank-badge';
      badge.textContent = '⚡ RERANK';
      resultCount.appendChild(badge);
    }
  }

  function formatRelativeTime(epochSeconds) {
    const timestampMs = epochSeconds * 1000;
    const now = Date.now();
    const diff = Math.max(0, now - timestampMs);
    const seconds = Math.floor(diff / 1000);

    if (seconds < 45) {
      return 'just now';
    }
    if (seconds < 90) {
      return 'a minute ago';
    }
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) {
      return `${minutes} min${minutes === 1 ? '' : 's'} ago`;
    }
    const hours = Math.floor(minutes / 60);
    if (hours < 24) {
      return `${hours} hour${hours === 1 ? '' : 's'} ago`;
    }
    const days = Math.floor(hours / 24);
    if (days < 7) {
      return `${days} day${days === 1 ? '' : 's'} ago`;
    }
    const weeks = Math.floor(days / 7);
    if (weeks < 5) {
      return `${weeks} week${weeks === 1 ? '' : 's'} ago`;
    }
    const months = Math.floor(days / 30);
    if (months < 12) {
      return `${months} month${months === 1 ? '' : 's'} ago`;
    }
    const years = Math.floor(days / 365);
    return `${years} year${years === 1 ? '' : 's'} ago`;
  }

  function formatBytes(bytes) {
    if (!bytes || bytes <= 0) {
      return '0 B';
    }
    const units = ['B', 'KB', 'MB', 'GB', 'TB'];
    const exponent = Math.min(units.length - 1, Math.floor(Math.log(bytes) / Math.log(1024)));
    const value = bytes / Math.pow(1024, exponent);
    return `${value.toFixed(value >= 10 || exponent === 0 ? 0 : 1)} ${units[exponent]}`;
  }
})();
