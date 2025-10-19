/**
 * Type definitions for ck search integration
 */

export type SearchMode = 'hybrid' | 'semantic' | 'regex';

export interface SearchResult {
  file: string;
  lineStart: number;
  lineEnd: number;
  byteStart: number;
  byteEnd: number;
  preview: string;
  score?: number;
  language?: string;
  absolutePath?: string; // Absolute path for opening (when file is relative)
}

export interface SearchOptions {
  query: string;
  mode: SearchMode;
  path: string;
  topK?: number;
  threshold?: number;
  pageSize?: number;
  caseInsensitive?: boolean;
  contextLines?: number;
  includePatterns?: string[];
  excludePatterns?: string[];
  rerank?: boolean;
  rerankModel?: string;
  respectGitignore?: boolean;
  useDefaultExcludes?: boolean;
  includeSnippet?: boolean;
  beforeContextLines?: number;
  afterContextLines?: number;
  cursor?: string;
  wholeWord?: boolean;
  fixedString?: boolean;
}

export interface IndexStatus {
  exists: boolean;
  path: string;
  totalFiles?: number;
  totalChunks?: number;
  embeddedChunks?: number;
  lastModified?: number;
  indexPath?: string;
  indexSizeBytes?: number;
  totalSizeBytes?: number;
  estimatedFileCount?: number;
  cacheHit?: boolean;
  model?: {
    name: string;
    alias: string;
    dimensions: number;
  };
}

export interface SearchResponse {
  results: SearchResult[];
  count: number;
  totalCount: number;
  hasMore: boolean;
  searchTime?: number;
  nextCursor?: string;
  searchTimeMs?: number;
}

export interface CkConfig {
  mode: 'cli' | 'mcp';
  cliPath: string;
  mcpCommand: string;
  mcpArgs: string[];
  indexRoot: string;
  defaultMode: SearchMode;
  pageSize: number;
  threshold: number;
  topK: number;
  contextLines: number;
}

export type IndexProgressSource = 'cli' | 'mcp';

export interface IndexProgressUpdate {
  message?: string;
  progress?: number;
  total?: number;
  source: IndexProgressSource;
  timestamp?: number;
}
