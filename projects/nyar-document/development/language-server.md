# 宿主语言 TypeScript 前端维护指南

本文档详细介绍基于 Nyar 编译器基础设施的 TypeScript 相关组件的维护和开发指南。

## 概述

基于 Nyar 编译器基础设施的 TypeScript 前端主要包括语言服务器 (LSP)、编辑器插件、调试器前端和开发工具。这些组件为宿主语言（如 Valkyrie）提供了完整的开发体验。

**重要说明**：Nyar 作为编译器本身不提供 LSP 服务，而是为宿主语言提供统一的 LSP 查询体系。

## 核心 TypeScript 组件

### 宿主语言 LSP 实现（如 valkyrie-lsp）

**职责**: 基于 Nyar 编译器基础设施，为宿主语言提供完整的语言服务器协议支持，包括语法高亮、代码补全、错误诊断、重构等功能。

**核心架构**:

```typescript
// 语言服务器主类
export class NyarLanguageServer {
    private connection: Connection;
    private documents: TextDocuments<TextDocument>;
    private workspaceManager: WorkspaceManager;
    private diagnosticManager: DiagnosticManager;
    private completionProvider: CompletionProvider;
    private hoverProvider: HoverProvider;
    private definitionProvider: DefinitionProvider;
    private renameProvider: RenameProvider;

    constructor() {
        this.connection = createConnection(ProposedFeatures.all);
        this.documents = new TextDocuments(TextDocument);
        this.setupHandlers();
    }

    private setupHandlers(): void {
        // 文档同步
        this.documents.onDidChangeContent(this.onDocumentChange.bind(this));
        this.documents.onDidClose(this.onDocumentClose.bind(this));

        // LSP 功能
        this.connection.onCompletion(this.onCompletion.bind(this));
        this.connection.onHover(this.onHover.bind(this));
        this.connection.onDefinition(this.onDefinition.bind(this));
        this.connection.onReferences(this.onReferences.bind(this));
        this.connection.onRenameRequest(this.onRename.bind(this));
        this.connection.onDocumentFormatting(this.onFormat.bind(this));
    }

    private async onDocumentChange(change: TextDocumentChangeEvent<TextDocument>): Promise<void> {
        const document = change.document;
        const diagnostics = await this.diagnosticManager.analyze(document);
        this.connection.sendDiagnostics({
            uri: document.uri,
            diagnostics
        });
    }
}
```

**维护要点**:
- 保持与最新 LSP 规范的兼容性
- 优化大文件的处理性能
- 实现增量解析和分析
- 提供准确的类型信息

### nyar-vscode-extension: VS Code 插件

**职责**: 为 VS Code 提供 Nyar 语言支持，包括语法高亮、调试、任务运行等功能。

**插件结构**:

```typescript
// 插件激活入口
export function activate(context: vscode.ExtensionContext): void {
    // 语言服务器客户端
    const serverOptions: ServerOptions = {
        run: {
            module: context.asAbsolutePath(path.join('server', 'out', 'server.js')),
            transport: TransportKind.ipc
        },
        debug: {
            module: context.asAbsolutePath(path.join('server', 'out', 'server.js')),
            transport: TransportKind.ipc,
            options: { execArgv: ['--nolazy', '--inspect=6009'] }
        }
    };

    const clientOptions: LanguageClientOptions = {
        documentSelector: [{ scheme: 'file', language: 'nyar' }],
        synchronize: {
            fileEvents: vscode.workspace.createFileSystemWatcher('**/*.ny')
        }
    };

    const client = new LanguageClient(
        'nyarLanguageServer',
        'Nyar Language Server',
        serverOptions,
        clientOptions
    );

    // 注册命令
    registerCommands(context);
    
    // 注册调试器
    registerDebugAdapter(context);
    
    // 注册任务提供者
    registerTaskProvider(context);
    
    // 启动语言服务器
    client.start();
    
    context.subscriptions.push(client);
}

// 命令注册
function registerCommands(context: vscode.ExtensionContext): void {
    const commands = [
        vscode.commands.registerCommand('nyar.run', runNyarFile),
        vscode.commands.registerCommand('nyar.debug', debugNyarFile),
        vscode.commands.registerCommand('nyar.test', testNyarFile),
        vscode.commands.registerCommand('nyar.build', buildNyarProject),
        vscode.commands.registerCommand('nyar.clean', cleanNyarProject),
    ];
    
    context.subscriptions.push(...commands);
}

// 调试适配器
class NyarDebugAdapterDescriptorFactory implements vscode.DebugAdapterDescriptorFactory {
    createDebugAdapterDescriptor(
        session: vscode.DebugSession,
        executable: vscode.DebugAdapterExecutable | undefined
    ): vscode.ProviderResult<vscode.DebugAdapterDescriptor> {
        return new vscode.DebugAdapterExecutable(
            'nyar-debugger',
            ['--dap'],
            {
                env: {
                    ...process.env,
                    NYAR_DEBUG_MODE: '1'
                }
            }
        );
    }
}
```

**维护要点**:
- 跟随 VS Code API 的更新
- 优化插件启动时间
- 提供丰富的配置选项
- 支持多工作区

### nyar-debugger-frontend: 调试器前端

**职责**: 实现调试适配器协议 (DAP)，提供断点、变量查看、调用栈等调试功能。

**调试器实现**:

```typescript
// 调试适配器
export class NyarDebugAdapter extends DebugAdapter {
    private runtime: NyarRuntime;
    private breakpoints: Map<string, Breakpoint[]> = new Map();
    private variableHandles = new Handles<Variable>();
    private stackFrameHandles = new Handles<StackFrame>();

    constructor() {
        super();
        this.runtime = new NyarRuntime();
        this.setupRuntimeHandlers();
    }

    private setupRuntimeHandlers(): void {
        this.runtime.on('stopOnEntry', () => {
            this.sendEvent(new StoppedEvent('entry', NyarDebugAdapter.THREAD_ID));
        });

        this.runtime.on('stopOnBreakpoint', () => {
            this.sendEvent(new StoppedEvent('breakpoint', NyarDebugAdapter.THREAD_ID));
        });

        this.runtime.on('stopOnException', (exception) => {
            this.sendEvent(new StoppedEvent('exception', NyarDebugAdapter.THREAD_ID, exception));
        });

        this.runtime.on('output', (text, category) => {
            this.sendEvent(new OutputEvent(text, category));
        });
    }

    protected async launchRequest(
        response: DebugProtocol.LaunchResponse,
        args: LaunchRequestArguments
    ): Promise<void> {
        try {
            await this.runtime.start(args.program, args.args || []);
            this.sendResponse(response);
        } catch (error) {
            this.sendErrorResponse(response, {
                id: 1001,
                format: `Failed to launch: ${error.message}`,
            });
        }
    }

    protected async setBreakPointsRequest(
        response: DebugProtocol.SetBreakpointsResponse,
        args: DebugProtocol.SetBreakpointsArguments
    ): Promise<void> {
        const path = args.source.path!;
        const clientLines = args.lines || [];
        
        // 清除现有断点
        this.runtime.clearBreakpoints(path);
        
        // 设置新断点
        const actualBreakpoints = await Promise.all(
            clientLines.map(async (line) => {
                const bp = await this.runtime.setBreakpoint(path, line);
                return new Breakpoint(bp.verified, line);
            })
        );
        
        this.breakpoints.set(path, actualBreakpoints);
        
        response.body = {
            breakpoints: actualBreakpoints
        };
        this.sendResponse(response);
    }

    protected async stackTraceRequest(
        response: DebugProtocol.StackTraceResponse,
        args: DebugProtocol.StackTraceArguments
    ): Promise<void> {
        const stack = await this.runtime.getStackTrace();
        const frames = stack.map((frame, index) => {
            const sf = new StackFrame(
                this.stackFrameHandles.create(frame),
                frame.name,
                new Source(frame.file),
                frame.line,
                frame.column
            );
            return sf;
        });

        response.body = {
            stackFrames: frames,
            totalFrames: frames.length
        };
        this.sendResponse(response);
    }

    protected async scopesRequest(
        response: DebugProtocol.ScopesResponse,
        args: DebugProtocol.ScopesArguments
    ): Promise<void> {
        const frame = this.stackFrameHandles.get(args.frameId);
        if (!frame) {
            this.sendErrorResponse(response, {
                id: 1002,
                format: 'Invalid frame ID'
            });
            return;
        }

        const scopes = await this.runtime.getScopes(frame);
        const scopeObjects = scopes.map(scope => new Scope(
            scope.name,
            this.variableHandles.create(scope.variables),
            scope.expensive
        ));

        response.body = {
            scopes: scopeObjects
        };
        this.sendResponse(response);
    }
}

// 运行时接口
interface NyarRuntime extends EventEmitter {
    start(program: string, args: string[]): Promise<void>;
    continue(): Promise<void>;
    step(): Promise<void>;
    stepIn(): Promise<void>;
    stepOut(): Promise<void>;
    pause(): Promise<void>;
    setBreakpoint(file: string, line: number): Promise<{ verified: boolean }>;
    clearBreakpoints(file: string): void;
    getStackTrace(): Promise<StackFrame[]>;
    getScopes(frame: StackFrame): Promise<Scope[]>;
    getVariables(scope: Scope): Promise<Variable[]>;
    evaluate(expression: string, frameId?: number): Promise<any>;
}
```

**维护要点**:
- 确保与 DAP 规范的兼容性
- 提供准确的调试信息
- 优化大型程序的调试性能
- 支持远程调试

### nyar-dev-tools: 开发工具集

**职责**: 提供项目管理、构建工具、包管理器等开发辅助功能。

**工具集架构**:

```typescript
// 项目管理器
export class ProjectManager {
    private projectRoot: string;
    private config: ProjectConfig;
    private dependencyManager: DependencyManager;
    private buildSystem: BuildSystem;

    constructor(projectRoot: string) {
        this.projectRoot = projectRoot;
        this.config = this.loadConfig();
        this.dependencyManager = new DependencyManager(this.config);
        this.buildSystem = new BuildSystem(this.config);
    }

    async createProject(template: ProjectTemplate): Promise<void> {
        await this.scaffoldProject(template);
        await this.installDependencies();
        await this.generateConfig();
    }

    async build(target?: BuildTarget): Promise<BuildResult> {
        const result = await this.buildSystem.build(target);
        if (result.success) {
            await this.runPostBuildTasks();
        }
        return result;
    }

    async test(pattern?: string): Promise<TestResult> {
        const testRunner = new TestRunner(this.config);
        return await testRunner.run(pattern);
    }

    async lint(): Promise<LintResult> {
        const linter = new NyarLinter(this.config);
        return await linter.check();
    }

    async format(): Promise<FormatResult> {
        const formatter = new NyarFormatter(this.config);
        return await formatter.format();
    }
}

// 构建系统
export class BuildSystem {
    private config: ProjectConfig;
    private compiler: NyarCompiler;
    private bundler: ModuleBundler;

    constructor(config: ProjectConfig) {
        this.config = config;
        this.compiler = new NyarCompiler(config.compiler);
        this.bundler = new ModuleBundler(config.bundler);
    }

    async build(target?: BuildTarget): Promise<BuildResult> {
        const startTime = Date.now();
        const errors: BuildError[] = [];
        const warnings: BuildWarning[] = [];

        try {
            // 编译源码
            const compileResult = await this.compiler.compile();
            errors.push(...compileResult.errors);
            warnings.push(...compileResult.warnings);

            if (compileResult.success) {
                // 打包模块
                const bundleResult = await this.bundler.bundle(target);
                errors.push(...bundleResult.errors);
                warnings.push(...bundleResult.warnings);

                // 生成输出
                if (bundleResult.success) {
                    await this.generateOutput(bundleResult.artifacts);
                }
            }

            return {
                success: errors.length === 0,
                errors,
                warnings,
                duration: Date.now() - startTime,
                artifacts: compileResult.artifacts
            };
        } catch (error) {
            return {
                success: false,
                errors: [{ message: error.message, file: '', line: 0, column: 0 }],
                warnings,
                duration: Date.now() - startTime,
                artifacts: []
            };
        }
    }
}

// 依赖管理器
export class DependencyManager {
    private config: ProjectConfig;
    private registry: PackageRegistry;
    private resolver: DependencyResolver;

    constructor(config: ProjectConfig) {
        this.config = config;
        this.registry = new PackageRegistry(config.registry);
        this.resolver = new DependencyResolver();
    }

    async install(packageName?: string): Promise<InstallResult> {
        if (packageName) {
            return await this.installPackage(packageName);
        } else {
            return await this.installAll();
        }
    }

    async update(packageName?: string): Promise<UpdateResult> {
        const packages = packageName ? [packageName] : this.getAllPackages();
        const results = await Promise.all(
            packages.map(pkg => this.updatePackage(pkg))
        );
        
        return {
            success: results.every(r => r.success),
            updated: results.filter(r => r.success).map(r => r.package),
            errors: results.filter(r => !r.success).map(r => r.error)
        };
    }

    async remove(packageName: string): Promise<RemoveResult> {
        const dependents = await this.findDependents(packageName);
        if (dependents.length > 0) {
            return {
                success: false,
                error: `Cannot remove ${packageName}: used by ${dependents.join(', ')}`
            };
        }

        await this.removePackage(packageName);
        await this.updateLockFile();
        
        return { success: true };
    }
}
```

**维护要点**:
- 提供直观的命令行界面
- 支持多种构建目标
- 实现高效的依赖解析
- 提供详细的错误信息

## 前端工具链集成

### Webpack 集成

```typescript
// Webpack 加载器
export class NyarWebpackLoader {
    static loader(source: string): string {
        const options = this.getOptions() || {};
        const compiler = new NyarCompiler(options);
        
        try {
            const result = compiler.compileToJavaScript(source);
            return result.code;
        } catch (error) {
            this.emitError(error);
            return '';
        }
    }
}

// Webpack 插件
export class NyarWebpackPlugin {
    private options: NyarPluginOptions;

    constructor(options: NyarPluginOptions = {}) {
        this.options = options;
    }

    apply(compiler: webpack.Compiler): void {
        compiler.hooks.compilation.tap('NyarWebpackPlugin', (compilation) => {
            // 添加 Nyar 文件处理
            compilation.hooks.buildModule.tap('NyarWebpackPlugin', (module) => {
                if (module.resource && module.resource.endsWith('.ny')) {
                    this.processNyarModule(module, compilation);
                }
            });

            // 生成类型定义文件
            compilation.hooks.processAssets.tap(
                {
                    name: 'NyarWebpackPlugin',
                    stage: webpack.Compilation.PROCESS_ASSETS_STAGE_ADDITIONAL
                },
                () => {
                    this.generateTypeDefinitions(compilation);
                }
            );
        });
    }
}
```

### Vite 集成

```typescript
// Vite 插件
export function nyarPlugin(options: NyarPluginOptions = {}): Plugin {
    return {
        name: 'vite-plugin-nyar',
        configResolved(config) {
            // 配置解析完成后的处理
        },
        buildStart() {
            // 构建开始时的处理
        },
        resolveId(id, importer) {
            if (id.endsWith('.ny')) {
                return id;
            }
        },
        load(id) {
            if (id.endsWith('.ny')) {
                return this.loadNyarFile(id);
            }
        },
        transform(code, id) {
            if (id.endsWith('.ny')) {
                return this.transformNyarCode(code, id);
            }
        },
        generateBundle(options, bundle) {
            // 生成 bundle 时的处理
        }
    };
}
```

### Rollup 集成

```typescript
// Rollup 插件
export function rollupNyarPlugin(options: NyarPluginOptions = {}): Plugin {
    return {
        name: 'rollup-plugin-nyar',
        resolveId(id, importer) {
            if (id.endsWith('.ny')) {
                return path.resolve(path.dirname(importer || ''), id);
            }
        },
        load(id) {
            if (id.endsWith('.ny')) {
                const source = fs.readFileSync(id, 'utf-8');
                const compiler = new NyarCompiler(options);
                const result = compiler.compileToJavaScript(source);
                return {
                    code: result.code,
                    map: result.sourceMap
                };
            }
        }
    };
}
```

## 测试框架

### 单元测试

```typescript
// Jest 配置
export const jestConfig: Config = {
    preset: 'ts-jest',
    testEnvironment: 'node',
    roots: ['<rootDir>/src', '<rootDir>/tests'],
    testMatch: [
        '**/__tests__/**/*.ts',
        '**/?(*.)+(spec|test).ts'
    ],
    transform: {
        '^.+\\.ts$': 'ts-jest',
        '^.+\\.ny$': '<rootDir>/jest-nyar-transformer.js'
    },
    collectCoverageFrom: [
        'src/**/*.ts',
        '!src/**/*.d.ts',
        '!src/generated/**'
    ],
    coverageReporters: ['text', 'lcov', 'html'],
    setupFilesAfterEnv: ['<rootDir>/tests/setup.ts']
};

// Nyar 文件转换器
export function nyarTransformer(source: string, filename: string): string {
    const compiler = new NyarCompiler({
        target: 'es2020',
        module: 'commonjs',
        sourceMap: true
    });
    
    const result = compiler.compileToJavaScript(source);
    return result.code;
}

// 测试工具
export class NyarTestUtils {
    static async compileAndRun(source: string): Promise<any> {
        const compiler = new NyarCompiler();
        const result = compiler.compile(source);
        
        if (result.errors.length > 0) {
            throw new Error(`Compilation failed: ${result.errors[0].message}`);
        }
        
        const vm = new NyarVM();
        return await vm.execute(result.bytecode);
    }
    
    static createMockRuntime(): MockNyarRuntime {
        return new MockNyarRuntime();
    }
    
    static async loadTestModule(path: string): Promise<any> {
        const source = await fs.promises.readFile(path, 'utf-8');
        return this.compileAndRun(source);
    }
}
```

### 集成测试

```typescript
// E2E 测试
describe('Nyar Language Server E2E', () => {
    let client: LanguageClient;
    let server: ChildProcess;

    beforeAll(async () => {
        // 启动语言服务器
        server = spawn('node', ['./out/server.js', '--stdio']);
        
        // 创建客户端连接
        client = new LanguageClient(
            'test-client',
            {
                run: { command: 'node', args: ['./out/server.js', '--stdio'] },
                debug: { command: 'node', args: ['./out/server.js', '--stdio'] }
            },
            {
                documentSelector: [{ scheme: 'file', language: 'nyar' }]
            }
        );
        
        await client.start();
    });

    afterAll(async () => {
        await client.stop();
        server.kill();
    });

    test('should provide completions', async () => {
        const document = TextDocument.create(
            'file:///test.ny',
            'nyar',
            1,
            'fn main() {\n    let x = \n}'
        );
        
        const completions = await client.sendRequest(
            'textDocument/completion',
            {
                textDocument: { uri: document.uri },
                position: { line: 1, character: 12 }
            }
        );
        
        expect(completions).toBeDefined();
        expect(completions.items.length).toBeGreaterThan(0);
    });

    test('should provide diagnostics', async () => {
        const document = TextDocument.create(
            'file:///test.ny',
            'nyar',
            1,
            'fn main() {\n    undefined_variable\n}'
        );
        
        // 等待诊断信息
        const diagnostics = await new Promise<Diagnostic[]>((resolve) => {
            client.onNotification('textDocument/publishDiagnostics', (params) => {
                if (params.uri === document.uri) {
                    resolve(params.diagnostics);
                }
            });
            
            client.sendNotification('textDocument/didOpen', {
                textDocument: document
            });
        });
        
        expect(diagnostics.length).toBeGreaterThan(0);
        expect(diagnostics[0].message).toContain('undefined');
    });
});
```

## 性能优化

### 语言服务器优化

```typescript
// 增量解析
export class IncrementalParser {
    private cache = new Map<string, ParseResult>();
    private dependencies = new Map<string, Set<string>>();

    async parse(uri: string, content: string): Promise<ParseResult> {
        const cached = this.cache.get(uri);
        if (cached && cached.version === this.getVersion(content)) {
            return cached;
        }

        const result = await this.doParse(content);
        result.version = this.getVersion(content);
        this.cache.set(uri, result);
        
        // 更新依赖关系
        this.updateDependencies(uri, result.imports);
        
        return result;
    }

    invalidate(uri: string): void {
        this.cache.delete(uri);
        
        // 递归失效依赖项
        const dependents = this.getDependents(uri);
        for (const dependent of dependents) {
            this.invalidate(dependent);
        }
    }
}

// 工作线程池
export class WorkerPool {
    private workers: Worker[] = [];
    private queue: Task[] = [];
    private busy = new Set<Worker>();

    constructor(size: number = os.cpus().length) {
        for (let i = 0; i < size; i++) {
            const worker = new Worker('./worker.js');
            this.workers.push(worker);
        }
    }

    async execute<T>(task: Task): Promise<T> {
        return new Promise((resolve, reject) => {
            const availableWorker = this.getAvailableWorker();
            if (availableWorker) {
                this.runTask(availableWorker, task, resolve, reject);
            } else {
                this.queue.push({ task, resolve, reject });
            }
        });
    }

    private getAvailableWorker(): Worker | null {
        return this.workers.find(w => !this.busy.has(w)) || null;
    }

    private runTask(
        worker: Worker,
        task: Task,
        resolve: Function,
        reject: Function
    ): void {
        this.busy.add(worker);
        
        worker.postMessage(task);
        
        const onMessage = (result: any) => {
            worker.off('message', onMessage);
            worker.off('error', onError);
            this.busy.delete(worker);
            this.processQueue();
            resolve(result);
        };
        
        const onError = (error: Error) => {
            worker.off('message', onMessage);
            worker.off('error', onError);
            this.busy.delete(worker);
            this.processQueue();
            reject(error);
        };
        
        worker.on('message', onMessage);
        worker.on('error', onError);
    }
}
```

### 内存优化

```typescript
// 对象池
export class ObjectPool<T> {
    private pool: T[] = [];
    private factory: () => T;
    private reset: (obj: T) => void;

    constructor(factory: () => T, reset: (obj: T) => void, initialSize = 10) {
        this.factory = factory;
        this.reset = reset;
        
        for (let i = 0; i < initialSize; i++) {
            this.pool.push(factory());
        }
    }

    acquire(): T {
        const obj = this.pool.pop();
        if (obj) {
            return obj;
        }
        return this.factory();
    }

    release(obj: T): void {
        this.reset(obj);
        this.pool.push(obj);
    }
}

// 弱引用缓存
export class WeakCache<K extends object, V> {
    private cache = new WeakMap<K, V>();
    private refs = new Set<WeakRef<K>>();
    private registry = new FinalizationRegistry<WeakRef<K>>((ref) => {
        this.refs.delete(ref);
    });

    set(key: K, value: V): void {
        this.cache.set(key, value);
        const ref = new WeakRef(key);
        this.refs.add(ref);
        this.registry.register(key, ref);
    }

    get(key: K): V | undefined {
        return this.cache.get(key);
    }

    has(key: K): boolean {
        return this.cache.has(key);
    }

    size(): number {
        return this.refs.size;
    }
}
```

## 部署和分发

### NPM 包配置

```json
{
  "name": "@nyar/language-tools",
  "version": "0.1.0",
  "description": "Nyar language tools for TypeScript/JavaScript",
  "main": "dist/index.js",
  "types": "dist/index.d.ts",
  "bin": {
    "valkyrie-lsp": "bin/valkyrie-lsp.js",
    "nyar-dev": "bin/nyar-dev.js"
  },
  "files": [
    "dist/",
    "bin/",
    "README.md",
    "LICENSE"
  ],
  "scripts": {
    "build": "tsc -p tsconfig.build.json",
    "test": "jest",
    "lint": "eslint src/**/*.ts",
    "format": "prettier --write src/**/*.ts",
    "prepublishOnly": "npm run build && npm test"
  },
  "dependencies": {
    "vscode-languageserver": "^8.0.0",
    "vscode-languageserver-textdocument": "^1.0.0",
    "vscode-uri": "^3.0.0"
  },
  "devDependencies": {
    "@types/node": "^18.0.0",
    "@types/jest": "^29.0.0",
    "typescript": "^4.9.0",
    "jest": "^29.0.0",
    "eslint": "^8.0.0",
    "prettier": "^2.8.0"
  },
  "engines": {
    "node": ">=16.0.0"
  },
  "repository": {
    "type": "git",
    "url": "https://github.com/nyar-lang/nyar-vm.git",
    "directory": "packages/language-tools"
  },
  "keywords": [
    "nyar",
    "language-server",
    "lsp",
    "typescript",
    "compiler"
  ],
  "license": "MIT"
}
```

### VS Code 插件配置

```json
{
  "name": "nyar-language-support",
  "displayName": "Nyar Language Support",
  "description": "Language support for Nyar programming language",
  "version": "0.1.0",
  "publisher": "nyar-lang",
  "engines": {
    "vscode": "^1.74.0"
  },
  "categories": [
    "Programming Languages",
    "Debuggers",
    "Formatters"
  ],
  "activationEvents": [
    "onLanguage:nyar"
  ],
  "main": "./out/extension.js",
  "contributes": {
    "languages": [
      {
        "id": "nyar",
        "aliases": ["Nyar", "nyar"],
        "extensions": [".ny"],
        "configuration": "./language-configuration.json"
      }
    ],
    "grammars": [
      {
        "language": "nyar",
        "scopeName": "source.nyar",
        "path": "./syntaxes/nyar.tmLanguage.json"
      }
    ],
    "commands": [
      {
        "command": "nyar.run",
        "title": "Run Nyar File",
        "category": "Nyar"
      },
      {
        "command": "nyar.debug",
        "title": "Debug Nyar File",
        "category": "Nyar"
      }
    ],
    "debuggers": [
      {
        "type": "nyar",
        "label": "Nyar Debug",
        "program": "./out/debugAdapter.js",
        "runtime": "node",
        "configurationAttributes": {
          "launch": {
            "required": ["program"],
            "properties": {
              "program": {
                "type": "string",
                "description": "Absolute path to a Nyar file.",
                "default": "${workspaceFolder}/main.ny"
              },
              "args": {
                "type": "array",
                "description": "Command line arguments passed to the program.",
                "default": []
              }
            }
          }
        }
      }
    ]
  },
  "scripts": {
    "vscode:prepublish": "npm run compile",
    "compile": "tsc -p ./",
    "watch": "tsc -watch -p ./",
    "package": "vsce package",
    "publish": "vsce publish"
  },
  "devDependencies": {
    "@types/vscode": "^1.74.0",
    "@vscode/test-electron": "^2.2.0",
    "vsce": "^2.15.0"
  }
}
```

## 维护最佳实践

### 代码质量

1. **类型安全**: 充分利用 TypeScript 的类型系统
2. **错误处理**: 使用 Result 类型或异常处理
3. **异步编程**: 正确使用 Promise 和 async/await
4. **内存管理**: 避免内存泄漏，使用弱引用

### 性能监控

1. **响应时间**: 监控 LSP 请求的响应时间
2. **内存使用**: 定期检查内存使用情况
3. **CPU 使用**: 避免阻塞主线程
4. **缓存效率**: 监控缓存命中率

### 用户体验

1. **错误信息**: 提供清晰、有用的错误信息
2. **进度反馈**: 长时间操作要显示进度
3. **配置选项**: 提供丰富的配置选项
4. **文档完整**: 维护完整的用户文档

### 兼容性

1. **版本兼容**: 保持向后兼容性
2. **平台支持**: 支持主流操作系统
3. **编辑器集成**: 支持多种编辑器
4. **工具链集成**: 与现有工具链良好集成

---

本文档将随着 Nyar 项目的发展持续更新。如有疑问或建议，请提交 Issue 或 Pull Request。