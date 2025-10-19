import { ChildProcessWithoutNullStreams, spawn } from 'child_process';
import * as path from 'path';
import * as readline from 'readline';
import * as vscode from 'vscode';
import packageJson from '../package.json';
import { CkConfig, IndexStatus, SearchOptions, SearchResponse, SearchResult, IndexProgressUpdate } from './types';

interface PendingRequest {
  resolve: (value: any) => void;
  reject: (error: Error) => void;
}

interface NotificationMessage {
  method?: string;
  params?: any;
}

export class CkMcpAdapter {
  private child: ChildProcessWithoutNullStreams | undefined;
  private readlineInterface: readline.Interface | undefined;
  private pendingRequests = new Map<number, PendingRequest>();
  private nextId = 1;
  private initializationPromise: Promise<void> | undefined;
  private outputChannel = vscode.window.createOutputChannel('ck MCP');
  private progressEmitter = new vscode.EventEmitter<IndexProgressUpdate>();

  public readonly onIndexProgress = this.progressEmitter.event;

  constructor(private readonly config: CkConfig, private readonly indexRoot: string) {}

  async search(options: SearchOptions): Promise<SearchResponse> {
    await this.ensureServer();

    const toolName = this.getToolName(options.mode);
    const resolvedPath = this.resolvePath(options.path);

    let requestBody: Record<string, unknown>;

    if (toolName === 'regex_search') {
      requestBody = {
        pattern: options.query,
        path: resolvedPath,
        ignore_case: options.caseInsensitive ?? false,
        context: options.contextLines,
        include_patterns: options.includePatterns,
        exclude_patterns: options.excludePatterns,
        respect_gitignore: options.respectGitignore ?? true,
        use_default_excludes: options.useDefaultExcludes ?? true,
        whole_word: options.wholeWord ?? false,
        fixed_string: options.fixedString ?? false,
        page_size: options.pageSize,
        include_snippet: options.includeSnippet ?? true,
        snippet_length: options.contextLines,
        cursor: options.cursor
      };
    } else {
      requestBody = {
        query: options.query,
        path: resolvedPath,
        top_k: options.topK,
        threshold: options.threshold,
        page_size: options.pageSize,
        include_patterns: options.includePatterns,
        exclude_patterns: options.excludePatterns,
        respect_gitignore: options.respectGitignore ?? true,
        use_default_excludes: options.useDefaultExcludes ?? true,
        rerank: options.rerank ?? (options.mode === 'semantic' || options.mode === 'hybrid'),
        rerank_model: options.rerankModel,
        case_insensitive: options.caseInsensitive ?? false,
        context_lines: options.contextLines,
        before_context_lines: options.beforeContextLines,
        after_context_lines: options.afterContextLines,
        include_snippet: options.includeSnippet ?? true,
        cursor: options.cursor
      };
    }

    const response = await this.callTool(toolName, requestBody);
    return this.transformSearchResponse(response);
  }

  async getIndexStatus(pathToCheck: string): Promise<IndexStatus> {
    await this.ensureServer();
    const response = await this.callTool('index_status', {
      path: this.resolvePath(pathToCheck)
    });

    const status = response?.index_status ?? response?.indexStatus ?? {};
    return {
      exists: Boolean(status.index_exists ?? status.indexed ?? false),
      path: status.path ?? pathToCheck,
      totalFiles: status.total_files,
      totalChunks: status.total_chunks,
      embeddedChunks: status.embedded_chunks,
      totalSizeBytes: status.total_size_bytes,
      lastModified: status.last_modified,
      indexPath: status.index_path,
      indexSizeBytes: status.index_size_bytes,
      estimatedFileCount: status.estimated_file_count,
      cacheHit: status.cache_hit,
      model: status.model ? {
        name: status.model.name,
        alias: status.model.alias,
        dimensions: status.model.dimensions
      } : undefined
    };
  }

  async reindex(
    pathToReindex: string,
    progress?: vscode.Progress<{ message?: string; increment?: number }>
  ): Promise<void> {
    await this.ensureServer();
    if (progress) {
      progress.report({ message: 'Starting MCP reindex…' });
    }
    await this.callTool('reindex', {
      path: this.resolvePath(pathToReindex),
      force: false
    });
    if (progress) {
      progress.report({ message: 'Reindex complete' });
    }
  }

  async getDefaultCkignoreContent(_indexRoot: string): Promise<string> {
    await this.ensureServer();
    const response = await this.callTool('default_ckignore', {});
    const content = response?.ckignore ?? response?.content ?? response;
    if (typeof content === 'string') {
      return content;
    }
    throw new Error('MCP server did not return default .ckignore content');
  }

  dispose(): void {
    this.shutdownChild('MCP server disposed');
    this.outputChannel.dispose();
    this.progressEmitter.dispose();
  }

  private resolvePath(targetPath: string): string {
    if (!targetPath) {
      return this.indexRoot;
    }
    if (path.isAbsolute(targetPath)) {
      return targetPath;
    }
    return path.join(this.indexRoot, targetPath);
  }

  private getToolName(mode: SearchOptions['mode']): string {
    switch (mode) {
      case 'semantic':
        return 'semantic_search';
      case 'hybrid':
        return 'hybrid_search';
      case 'regex':
        return 'regex_search';
      default:
        return 'lexical_search';
    }
  }

  private async ensureServer(): Promise<void> {
    if (this.initializationPromise) {
      return this.initializationPromise;
    }

    if (!this.child || this.child.killed) {
      this.spawnServer();
    }

    this.initializationPromise = this.initializeServer();
    return this.initializationPromise;
  }

  private spawnServer(): void {
    this.shutdownChild('MCP server restarted');
    const command = this.config.mcpCommand || this.config.cliPath || 'ck';
    const args = this.config.mcpArgs && this.config.mcpArgs.length > 0 ? this.config.mcpArgs : ['--serve'];

    this.child = spawn(command, args, {
      cwd: this.indexRoot,
      stdio: ['pipe', 'pipe', 'pipe']
    });

    this.child.on('error', (err) => {
      vscode.window.showErrorMessage(`ck MCP server failed: ${err.message}`);
      this.rejectAllPending(err);
    });

    this.child.on('exit', (code, signal) => {
      const message = code !== null ? `exit code ${code}` : `signal ${signal}`;
      this.outputChannel.appendLine(`[ck-mcp] server exited (${message})`);
      this.rejectAllPending(new Error(`MCP server exited (${message})`));
      this.child = undefined;
      this.initializationPromise = undefined;
    });

    if (this.child.stderr) {
      this.child.stderr.setEncoding('utf8');
      this.child.stderr.on('data', (data: string) => {
        data
          .split(/\r?\n/)
          .filter(Boolean)
          .forEach((line) => this.outputChannel.appendLine(`[stderr] ${line}`));
      });
    }

    if (this.child.stdout) {
      this.child.stdout.setEncoding('utf8');
      this.readlineInterface = readline.createInterface({ input: this.child.stdout });
      this.readlineInterface.on('line', (line) => this.handleLine(line));
    }

    this.outputChannel.appendLine('[ck-mcp] server spawned');
  }

  private async initializeServer(): Promise<void> {
    try {
      const protocolVersion = '2024-11-05';
      await this.sendRequestInternal('initialize', {
        protocolVersion,
        capabilities: {},
        clientInfo: {
          name: 'ck-vscode',
          version: packageJson.version ?? '0.0.0'
        }
      });
      await this.sendNotification('notifications/initialized', {});
      this.outputChannel.appendLine('[ck-mcp] initialization complete');
    } catch (error) {
      this.outputChannel.appendLine(`[ck-mcp] initialization failed: ${(error as Error).message}`);
      this.shutdownChild('MCP server initialization failed');
      throw error;
    }
  }

  private async callTool(name: string, args: Record<string, unknown>): Promise<any> {
    const rawResult = await this.sendRequest('tools/call', {
      name,
      arguments: args
    });

    const isError = Boolean(rawResult?.is_error ?? rawResult?.isError);
    if (isError) {
      const message = typeof rawResult === 'object' && rawResult?.content?.[0]?.text
        ? rawResult.content[0].text
        : 'Unknown MCP error';
      throw new Error(message);
    }

    return rawResult?.structured_content ?? rawResult?.structuredContent ?? rawResult;
  }

  private transformSearchResponse(raw: any): SearchResponse {
    const matches = Array.isArray(raw?.results?.matches) ? raw.results.matches : [];
    const results: SearchResult[] = matches.map((match: any) => {
      const filePath = match?.file?.path ?? '';
      const span = match?.match?.span ?? {};
      const lineStart = span.line_start ?? 1;
      const lineEnd = span.line_end ?? lineStart;
      return {
        file: filePath,
        lineStart,
        lineEnd,
        byteStart: span.byte_start ?? 0,
        byteEnd: span.byte_end ?? 0,
        preview: match?.match?.content ?? '',
        score: match?.match?.score,
        language: match?.file?.language ?? undefined,
        absolutePath: filePath
      };
    });

    return {
      results,
      count: raw?.results?.count ?? results.length,
      totalCount: raw?.results?.total_count ?? results.length,
      hasMore: Boolean(raw?.results?.has_more),
      nextCursor: raw?.pagination?.next_cursor ?? undefined,
      searchTimeMs: raw?.metadata?.search_time_ms
    };
  }

  private async sendRequest(method: string, params?: Record<string, unknown>): Promise<any> {
    await this.ensureServer();
    return this.sendRequestInternal(method, params ?? {});
  }

  private sendRequestInternal(method: string, params?: Record<string, unknown>): Promise<any> {
    if (!this.child || !this.child.stdin) {
      throw new Error('MCP server is not running');
    }

    const id = this.nextId++;
    const payload = {
      jsonrpc: '2.0',
      id,
      method,
      params
    };

    return new Promise((resolve, reject) => {
      this.pendingRequests.set(id, { resolve, reject });
      this.writeMessage(payload).catch((error) => {
        this.pendingRequests.delete(id);
        reject(error);
      });
    });
  }

  private async sendNotification(method: string, params?: Record<string, unknown>): Promise<void> {
    const payload = {
      jsonrpc: '2.0',
      method,
      params
    };

    try {
      await this.writeMessage(payload);
    } catch (error) {
      this.outputChannel.appendLine(`[ck-mcp] failed to send notification ${method}: ${(error as Error).message}`);
      throw error;
    }
  }

  private handleLine(line: string): void {
    const trimmed = line.trim();
    if (!trimmed) {
      return;
    }

    let message: any;
    try {
      message = JSON.parse(trimmed);
    } catch (error) {
      this.outputChannel.appendLine(`[ck-mcp] failed to parse JSON: ${trimmed}`);
      return;
    }

    if (typeof message.id !== 'undefined') {
      const pending = this.pendingRequests.get(message.id);
      if (!pending) {
        return;
      }
      this.pendingRequests.delete(message.id);

      if (Object.prototype.hasOwnProperty.call(message, 'error')) {
        const errorData = message.error?.data?.details ?? message.error?.message ?? 'Unknown MCP error';
        pending.reject(new Error(errorData));
      } else {
        pending.resolve(message.result);
      }
      return;
    }

    this.handleNotification(message);
  }

  private handleNotification(notification: NotificationMessage): void {
    if (!notification?.method) {
      return;
    }

    switch (notification.method) {
      case '$/progress':
        if (notification.params?.message) {
          this.outputChannel.appendLine(`[ck-mcp] ${notification.params.message}`);
        }
        this.progressEmitter.fire({
          message: notification.params?.message,
          progress: notification.params?.progress,
          total: notification.params?.total,
          source: 'mcp',
          timestamp: Date.now()
        });
        break;
      default:
        this.outputChannel.appendLine(`[ck-mcp] notification ${notification.method}`);
    }
  }

  private rejectAllPending(error: Error): void {
    this.pendingRequests.forEach(({ reject }) => reject(error));
    this.pendingRequests.clear();
  }

  private shutdownChild(message?: string): void {
    if (this.readlineInterface) {
      this.readlineInterface.close();
      this.readlineInterface = undefined;
    }
    if (this.child && !this.child.killed) {
      this.child.kill();
    }
    this.child = undefined;
    this.initializationPromise = undefined;
    if (this.pendingRequests.size > 0) {
      const error = new Error(message ?? 'MCP server stopped');
      this.rejectAllPending(error);
    }
  }

  private writeMessage(payload: Record<string, unknown>): Promise<void> {
    if (!this.child || !this.child.stdin) {
      return Promise.reject(new Error('MCP server is not running'));
    }

    return new Promise((resolve, reject) => {
      this.child!.stdin.write(`${JSON.stringify(payload)}\n`, (err) => {
        if (err) {
          reject(err);
        } else {
          resolve();
        }
      });
    });
  }
}
