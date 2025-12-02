import * as vscode from "vscode";
import type { Range } from "vscode-languageserver-types";

import { bootstrapFerrousOwl, installFerrousOwl } from "./bootstrap";
import {
  LanguageClient,
  type Executable,
  type LanguageClientOptions,
  State,
  ErrorAction,
  CloseAction,
  type ErrorHandler,
} from "vscode-languageclient/node";

type DisplayMode = "selected" | "manual" | "disabled";

type LspDecorationType =
  | "lifetime"
  | "imm_borrow"
  | "mut_borrow"
  | "move"
  | "call"
  | "shared_mut"
  | "outlive";

interface LspDecoration {
  readonly type: LspDecorationType;
  readonly range: Range;
  readonly hover_text?: string | null;
  readonly overlapped: boolean;
}

interface LspCursorResponse {
  readonly is_analyzed: boolean;
  readonly status: "analyzing" | "finished" | "error";
  readonly decorations: readonly LspDecoration[];
}

export let client: LanguageClient | undefined = undefined;

let decoTimer: NodeJS.Timeout | null = null;

const getConfig = (): vscode.WorkspaceConfiguration =>
  vscode.workspace.getConfiguration("ferrous-owl");

const getDisplayMode = (): DisplayMode =>
  getConfig().get<DisplayMode>("displayMode", "selected");

const getDisplayDelay = (): number =>
  getConfig().get<number>("displayDelay", 500);

interface DecorationConfig {
  readonly underlineThickness: string;
  readonly lifetimeColor: string;
  readonly moveCallColor: string;
  readonly immutableBorrowColor: string;
  readonly mutableBorrowColor: string;
  readonly sharedMutColor: string;
  readonly outliveColor: string;
  readonly highlightBackground: boolean;
}

const getDecorationConfig = (): DecorationConfig => {
  const config = getConfig();
  return {
    underlineThickness: config.get<string>("underlineThickness", "2"),
    lifetimeColor: config.get<string>("lifetimeColor", "hsla(125, 80%, 60%, 0.6)"),
    moveCallColor: config.get<string>("moveCallColor", "hsla(35, 80%, 60%, 0.6)"),
    immutableBorrowColor: config.get<string>("immutableBorrowColor", "hsla(230, 80%, 60%, 0.6)"),
    mutableBorrowColor: config.get<string>("mutableBorrowColor", "hsla(300, 80%, 60%, 0.6)"),
    sharedMutColor: config.get<string>("sharedMutColor", "hsla(60, 80%, 50%, 0.6)"),
    outliveColor: config.get<string>("outliveColor", "hsla(0, 80%, 60%, 0.6)"),
    highlightBackground: config.get<boolean>("highlightBackground", false),
  };
};

const lspRangeToVscodeRange = (range: Range): vscode.Range =>
  new vscode.Range(
    new vscode.Position(range.start.line, range.start.character),
    new vscode.Position(range.end.line, range.end.character),
  );

const createDecorationType = (
  color: string,
  thickness: string,
  useBackground: boolean,
): vscode.TextEditorDecorationType =>
  useBackground
    ? vscode.window.createTextEditorDecorationType({ backgroundColor: color })
    : vscode.window.createTextEditorDecorationType({
        textDecoration: `underline solid ${thickness}px ${color}`,
      });

const categorizeDecoration = (
  deco: LspDecoration,
): "lifetime" | "immut" | "mut" | "moveCall" | "sharedMut" | "outlive" => {
  switch (deco.type) {
    case "lifetime":
      return "lifetime";
    case "imm_borrow":
      return "immut";
    case "mut_borrow":
      return "mut";
    case "call":
    case "move":
      return "moveCall";
    case "shared_mut":
      return "sharedMut";
    case "outlive":
      return "outlive";
  }
};

class DecorationManager {
  private lifetimeDecorationType = vscode.window.createTextEditorDecorationType({});
  private moveDecorationType = vscode.window.createTextEditorDecorationType({});
  private imBorrowDecorationType = vscode.window.createTextEditorDecorationType({});
  private mBorrowDecorationType = vscode.window.createTextEditorDecorationType({});
  private sharedMutDecorationType = vscode.window.createTextEditorDecorationType({});
  private outLiveDecorationType = vscode.window.createTextEditorDecorationType({});
  private emptyDecorationType = vscode.window.createTextEditorDecorationType({});

  public dispose(): void {
    this.lifetimeDecorationType.dispose();
    this.moveDecorationType.dispose();
    this.imBorrowDecorationType.dispose();
    this.mBorrowDecorationType.dispose();
    this.sharedMutDecorationType.dispose();
    this.outLiveDecorationType.dispose();
    this.emptyDecorationType.dispose();
  }

  public update(editor: vscode.TextEditor, data: LspCursorResponse): void {
    this.dispose();

    const config = getDecorationConfig();
    this.lifetimeDecorationType = createDecorationType(config.lifetimeColor, config.underlineThickness, config.highlightBackground);
    this.moveDecorationType = createDecorationType(config.moveCallColor, config.underlineThickness, config.highlightBackground);
    this.imBorrowDecorationType = createDecorationType(config.immutableBorrowColor, config.underlineThickness, config.highlightBackground);
    this.mBorrowDecorationType = createDecorationType(config.mutableBorrowColor, config.underlineThickness, config.highlightBackground);
    this.sharedMutDecorationType = createDecorationType(config.sharedMutColor, config.underlineThickness, config.highlightBackground);
    this.outLiveDecorationType = createDecorationType(config.outliveColor, config.underlineThickness, config.highlightBackground);
    this.emptyDecorationType = vscode.window.createTextEditorDecorationType({});

    const grouped = {
      lifetime: [] as vscode.DecorationOptions[],
      immut: [] as vscode.DecorationOptions[],
      mut: [] as vscode.DecorationOptions[],
      moveCall: [] as vscode.DecorationOptions[],
      sharedMut: [] as vscode.DecorationOptions[],
      outlive: [] as vscode.DecorationOptions[],
      messages: [] as vscode.DecorationOptions[],
    };

    for (const deco of data.decorations) {
      const range = lspRangeToVscodeRange(deco.range);

      if (!deco.overlapped) {
        const category = categorizeDecoration(deco);
        grouped[category].push({ range });
      }

      if (deco.hover_text) {
        grouped.messages.push({ range, hoverMessage: deco.hover_text });
      }
    }

    editor.setDecorations(this.lifetimeDecorationType, grouped.lifetime);
    editor.setDecorations(this.imBorrowDecorationType, grouped.immut);
    editor.setDecorations(this.mBorrowDecorationType, grouped.mut);
    editor.setDecorations(this.moveDecorationType, grouped.moveCall);
    editor.setDecorations(this.sharedMutDecorationType, grouped.sharedMut);
    editor.setDecorations(this.outLiveDecorationType, grouped.outlive);
    editor.setDecorations(this.emptyDecorationType, grouped.messages);
  }
}

class StatusBarManager {
  private readonly statusBar: vscode.StatusBarItem;

  constructor() {
    this.statusBar = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 0);
    this.statusBar.text = "FerrousOwl";
    this.statusBar.command = {
      command: "ferrous-owl.cycleDisplayMode",
      title: "Cycle display mode",
      tooltip: "Cycle FerrousOwl display mode",
    };
    this.statusBar.show();
  }

  public updateFromLspStatus(status: string): void {
    switch (status) {
      case "finished":
        this.statusBar.text = "$(check) FerrousOwl";
        this.statusBar.tooltip = "Analysis finished";
        break;
      case "analyzing":
        this.statusBar.text = "$(loading~spin) FerrousOwl";
        this.statusBar.tooltip = "Analyzing...";
        break;
      default:
        this.statusBar.text = "$(error) FerrousOwl";
        this.statusBar.tooltip = "Analysis failed";
    }
    this.statusBar.show();
  }

  public updateFromDisplayMode(mode: DisplayMode): void {
    const modeConfig: Record<DisplayMode, { icon: string; tooltip: string }> = {
      selected: { icon: "check", tooltip: "Display mode: selected" },
      manual: { icon: "tools", tooltip: "Display mode: manual" },
      disabled: { icon: "debug-pause", tooltip: "Display mode: disabled" },
    };
    const config = modeConfig[mode];
    this.statusBar.text = `$(${config.icon}) FerrousOwl`;
    this.statusBar.tooltip = config.tooltip;
    this.statusBar.show();
  }

  public dispose(): void {
    this.statusBar.dispose();
  }
}

interface ServerState {
  lastError: string | null;
  startCount: number;
}

const createErrorHandler = (
  statusBarManager: StatusBarManager,
  outputChannel: vscode.LogOutputChannel,
  serverState: ServerState,
  command: string,
): ErrorHandler => ({
  error: (error, message, count) => {
    const errorCount = count ?? 0;
    serverState.lastError = `${error.name}: ${error.message}`;
    
    outputChannel.error(`LSP error #${errorCount}: ${error.name}`);
    outputChannel.error(`  Message: ${error.message}`);
    if (message) {
      outputChannel.error(`  LSP Message: ${JSON.stringify(message)}`);
    }
    outputChannel.error(`  Stack: ${error.stack ?? "N/A"}`);
    
    console.error(`FerrousOwl LSP error (${errorCount}):`, error, message);
    statusBarManager.updateFromLspStatus("error");
    
    if (errorCount >= 3) {
      void vscode.window.showErrorMessage(
        `FerrousOwl server keeps failing: ${serverState.lastError}. ` +
        "Try running 'FerrousOwl: Update' command to rebuild.",
        "Show Logs",
        "Update"
      ).then((choice) => {
        if (choice === "Show Logs") {
          outputChannel.show();
        } else if (choice === "Update") {
            void vscode.commands.executeCommand("ferrous-owl.update");
        }
      });
      return { action: ErrorAction.Shutdown };
    }
    return { action: ErrorAction.Continue };
  },
  closed: () => {
    serverState.startCount++;
    const errorInfo = serverState.lastError ? ` Last error: ${serverState.lastError}` : "";
    
    outputChannel.warn(`Server connection closed (attempt ${serverState.startCount}).${errorInfo}`);
    outputChannel.warn(`Server command: ${command}`);
    
    console.warn(`FerrousOwl LSP connection closed (attempt ${serverState.startCount}).${errorInfo}`);
    statusBarManager.updateFromLspStatus("error");
    
    if (serverState.startCount >= 3) {
      void vscode.window.showErrorMessage(
        `FerrousOwl server crashed ${serverState.startCount} times.${errorInfo} ` +
        "The server binary may be corrupted or incompatible.",
        "Show Logs",
        "Update Server"
      ).then((choice) => {
        if (choice === "Show Logs") {
          outputChannel.show();
        } else if (choice === "Update Server") {
          void vscode.commands.executeCommand("ferrous-owl.update");
        }
      });
      return { action: CloseAction.DoNotRestart };
    }
    
    void vscode.window.showWarningMessage(
      `FerrousOwl server stopped unexpectedly.${errorInfo} Restart?`,
      "Restart",
      "Show Logs",
      "Ignore"
    ).then((choice) => {
      if (choice === "Restart") {
        void vscode.commands.executeCommand("ferrous-owl.restart");
      } else if (choice === "Show Logs") {
        outputChannel.show();
      }
    });
    
    return { action: CloseAction.DoNotRestart };
  },
});

const initializeClient = async (
  context: vscode.ExtensionContext,
  clientOptions: LanguageClientOptions,
  statusBarManager: StatusBarManager,
): Promise<void> => {
  const command = await bootstrapFerrousOwl(
    context.extensionPath,
    context.extensionMode,
  );
  
  const outputChannel = vscode.window.createOutputChannel("FerrousOwl Server", { log: true });
  context.subscriptions.push(outputChannel);

  const isDevelopment = context.extensionMode === vscode.ExtensionMode.Development;
  
  const serverOptions: Executable = {
    command,
    options: {
      env: {
        ...process.env,
        RUST_LOG: isDevelopment ? "debug" : "info",
      },
    },
  };

  const serverState: ServerState = { lastError: null, startCount: 0 };
  const errorHandler = createErrorHandler(statusBarManager, outputChannel, serverState, command);

  const fullClientOptions: LanguageClientOptions = {
    ...clientOptions,
    errorHandler,
    outputChannel,
    traceOutputChannel: outputChannel,
  };

  client = new LanguageClient("ferrous-owl", "FerrousOwl", serverOptions, fullClientOptions);
  
  outputChannel.info(`Starting FerrousOwl server: ${command}`);
  
  client.onDidChangeState((e) => {
    const stateNames = new Map<number, string>([
      [1, "stopped"],
      [2, "running"],
      [3, "starting"],
    ]);
    const oldName = stateNames.get(e.oldState) ?? String(e.oldState);
    const newName = stateNames.get(e.newState) ?? String(e.newState);
    
    outputChannel.info(`Client state: ${oldName} -> ${newName}`);
    console.warn(`FerrousOwl client state: ${oldName} -> ${newName}`);
    
    if (e.newState === State.Running) {
      serverState.startCount = 0;
      serverState.lastError = null;
      statusBarManager.updateFromDisplayMode(getDisplayMode());
    }
  });
  
  await client.start();
};

const isLspCursorResponse = (value: unknown): value is LspCursorResponse =>
  typeof value === "object" &&
  value !== null &&
  "is_analyzed" in value &&
  "status" in value &&
  "decorations" in value &&
  Array.isArray((value as LspCursorResponse).decorations);

const sendCursorRequest = async (
  position: vscode.Position,
  uri: vscode.Uri,
): Promise<LspCursorResponse | null> => {
  const resp = await client?.sendRequest("ferrous-owl/cursor", {
    position: { line: position.line, character: position.character },
    document: { uri: uri.toString() },
  });
  return isLspCursorResponse(resp) ? resp : null;
};

const createClientWithOptions = (
  command: string,
  clientOptions: LanguageClientOptions,
): LanguageClient =>
  new LanguageClient("ferrous-owl", "FerrousOwl", {
    command,
  } satisfies Executable, clientOptions);

const restartClient = async (
  clientOptions: LanguageClientOptions,
  extensionPath: string,
  extensionMode?: vscode.ExtensionMode,
): Promise<void> => {
  if (client?.isRunning()) {
    await client.stop();
  }
  client = undefined;

  const binary = await bootstrapFerrousOwl(extensionPath, extensionMode);
  client = createClientWithOptions(binary, clientOptions);
  await client.start();
};

const updateAndRestartClient = async (clientOptions: LanguageClientOptions): Promise<void> => {
  if (client?.isRunning()) {
    await client.stop();
  }
  client = undefined;

  const newBinary = await installFerrousOwl();
  client = createClientWithOptions(newBinary, clientOptions);
  await client.start();
};

const registerCommands = (
  context: vscode.ExtensionContext,
  activeEditorRef: { current: vscode.TextEditor | undefined },
  decorationManager: DecorationManager,
  statusBarManager: StatusBarManager,
  clientOptions: LanguageClientOptions,
): void => {
  const handleCursorRequest = async (
    editor: vscode.TextEditor,
    position: vscode.Position,
  ): Promise<void> => {
    const data = await sendCursorRequest(position, editor.document.uri);
    if (data) {
      statusBarManager.updateFromLspStatus(data.status);
      decorationManager.update(editor, data);
    }
  };

  const registerCommand = (
    id: string,
    handler: (...args: never[]) => unknown,
  ): void => {
    context.subscriptions.push(vscode.commands.registerCommand(id, handler));
  };

  registerCommand("ferrous-owl.hover", async () => {
    const editor = activeEditorRef.current;
    if (editor) {
      await handleCursorRequest(editor, editor.selection.active);
    }
  });

  registerCommand("ferrous-owl.cycleDisplayMode", async () => {
    const modes: DisplayMode[] = ["selected", "manual", "disabled"];
    const current = getDisplayMode();
    const nextMode = modes[(modes.indexOf(current) + 1) % modes.length];
    await getConfig().update("displayMode", nextMode, vscode.ConfigurationTarget.Global);
    void vscode.window.showInformationMessage(`FerrousOwl display mode: ${nextMode}`);
  });

  registerCommand("ferrous-owl.toggle", async () => {
    const current = getDisplayMode();
    const newMode = current === "disabled" ? "selected" : "disabled";
    await getConfig().update("displayMode", newMode, vscode.ConfigurationTarget.Global);
    void vscode.window.showInformationMessage(
      `FerrousOwl ${newMode === "disabled" ? "disabled" : "enabled"}`,
    );
  });

  // Note: ferrous-owl.toggleOwnership, ferrous-owl.analyze, and other ownership commands
  // are registered by the LSP client from server capabilities.
  // The middleware in clientOptions handles injecting the current position.

  registerCommand("ferrous-owl.restart", async () => {
    try {
      await restartClient(clientOptions, context.extensionPath, context.extensionMode);
      void vscode.window.showInformationMessage("FerrousOwl restarted successfully!");
    } catch (e) {
      void vscode.window.showErrorMessage(`Failed to restart FerrousOwl: ${String(e)}`);
    }
  });

  registerCommand("ferrous-owl.update", async () => {
    const choice = await vscode.window.showWarningMessage(
      "This will stop the current FerrousOwl server and rebuild. Continue?",
      "Yes",
      "No"
    );
    
    if (choice !== "Yes") {
      return;
    }

    try {
      await updateAndRestartClient(clientOptions);
      void vscode.window.showInformationMessage("FerrousOwl updated and restarted successfully!");
    } catch (e) {
      void vscode.window.showErrorMessage(`Failed to update FerrousOwl: ${String(e)}`);
    }
  });
};

const registerEventHandlers = (
  context: vscode.ExtensionContext,
  activeEditorRef: { current: vscode.TextEditor | undefined },
  decorationManager: DecorationManager,
  statusBarManager: StatusBarManager,
): void => {
  const triggerDecoration = (editor: vscode.TextEditor, position: vscode.Position): void => {
    decorationManager.dispose();

    if (decoTimer) {
      clearTimeout(decoTimer);
      decoTimer = null;
    }

    decoTimer = setTimeout(() => {
      void (async (): Promise<void> => {
        const data = await sendCursorRequest(position, editor.document.uri);
        if (data) {
          statusBarManager.updateFromLspStatus(data.status);
          decorationManager.update(editor, data);
        }
      })();
    }, getDisplayDelay());
  };

  context.subscriptions.push(
    vscode.window.onDidChangeActiveTextEditor((editor) => {
      activeEditorRef.current = editor;
    }),

    vscode.workspace.onDidSaveTextDocument((doc) => {
      if (doc.languageId === "rust") {
        void client?.sendRequest("ferrous-owl/analyze", {});
      }
    }),

    vscode.window.onDidChangeTextEditorSelection((ev) => {
      const mode = getDisplayMode();
      if (
        mode === "selected" &&
        ev.textEditor === activeEditorRef.current &&
        ev.textEditor.document.languageId === "rust"
      ) {
        triggerDecoration(ev.textEditor, ev.textEditor.selection.active);
      }
    }),

    vscode.workspace.onDidChangeConfiguration((ev) => {
      if (ev.affectsConfiguration("ferrous-owl.displayMode")) {
        const mode = getDisplayMode();
        statusBarManager.updateFromDisplayMode(mode);
        if (mode === "disabled") {
          decorationManager.dispose();
        }
      }
    }),
  );
};

export function activate(context: vscode.ExtensionContext): void {
  const activeEditorRef = { current: vscode.window.activeTextEditor };
  
  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "rust" }],
    middleware: {
      // Intercept executeCommand to inject current position for ownership commands
      executeCommand: async (command: string, args: unknown[], next) => {
        if (
          (command === "ferrous-owl.toggleOwnership" ||
           command === "ferrous-owl.enableOwnership" ||
           command === "ferrous-owl.disableOwnership") &&
          args.length === 0 &&
          activeEditorRef.current
        ) {
          const editor = activeEditorRef.current;
          const newArgs = [
            editor.document.uri.toString(),
            editor.selection.active.line,
            editor.selection.active.character,
          ];
          return next(command, newArgs) as Promise<unknown>;
        }
        return next(command, args) as Promise<unknown>;
      },
    },
  };

  const decorationManager = new DecorationManager();
  const statusBarManager = new StatusBarManager();

  context.subscriptions.push({ dispose: () => decorationManager.dispose() });
  context.subscriptions.push({ dispose: () => statusBarManager.dispose() });

  void initializeClient(context, clientOptions, statusBarManager).catch((e: unknown) => {
    void vscode.window.showErrorMessage(`Failed to start FerrousOwl\n${String(e)}`);
  });

  registerCommands(context, activeEditorRef, decorationManager, statusBarManager, clientOptions);
  registerEventHandlers(context, activeEditorRef, decorationManager, statusBarManager);
}

export async function deactivate(): Promise<void> {
  if (client?.isRunning()) {
    await client.stop();
  }
  client = undefined;
}
