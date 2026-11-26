// KompozR in VS Code. KompozR's UI code is Kotlin: inside kompoz! { … } blocks of Rust sources, and in whole .kt
// files read by kompoz_file!. The Kotlin is highlighted as Kotlin (a grammar of .kt files' own, and one injected
// into Rust's for kompoz!), and kompoz-lsp, KompozR's language server, answers for it: the generator's errors on
// the Kotlin's lines (rustc's too, through its shadow-project bridge), hovers, completion, going to definitions.
// In front of the toolchain's rust-analyzer it answers for the Rust around it as well, unless the rust-analyzer
// extension is installed, which then keeps the Rust (kompozr.rustAnalyzer.mode).

import * as path from "path";
import * as vscode from "vscode";
import { LanguageClient, LanguageClientOptions, ServerOptions, State } from "vscode-languageclient/node";
import { findServer, onPath, proxiesRust, RustAnalyzerMode, serverArguments, startsServer } from "./server";

const rustAnalyzerExtension = "rust-lang.rust-analyzer";
let client: LanguageClient | undefined;
let starting: Promise<void> | undefined;
let output: vscode.OutputChannel;
const pending = new Map<string, NodeJS.Timeout>();  // a document's check after its edits, by URI

export function activate(context: vscode.ExtensionContext): void {
    output = vscode.window.createOutputChannel("KompozR");
    context.subscriptions.push(
        output,
        vscode.workspace.onDidOpenTextDocument(check),
        // (After a pause in the typing: a Rust source gains its first kompoz!.)
        vscode.workspace.onDidChangeTextDocument((e) => {
            if (client || e.document.languageId !== "rust") return;
            const key = e.document.uri.toString();
            clearTimeout(pending.get(key));
            pending.set(key, setTimeout(() => {
                pending.delete(key);
                check(e.document);
            }, 500));
        }),
        vscode.workspace.onDidChangeConfiguration((e) => {
            if (e.affectsConfiguration("kompozr") && !e.affectsConfiguration("kompozr.trace") && client) void restart();
        }),
        // (The rust-analyzer extension installed or removed: in the auto mode, who has the Rust changes.)
        vscode.extensions.onDidChange(() => {
            if (client && mode() === "auto" && proxying() !== runningProxied) void restart();
        }),
        vscode.commands.registerCommand("kompozr.restartServer", () => restart(true)),
        { dispose: () => void client?.stop() },
    );
    vscode.workspace.textDocuments.forEach(check);
}

export function deactivate(): Thenable<void> | undefined {
    return client?.stop();
}

function settings(): vscode.WorkspaceConfiguration {
    return vscode.workspace.getConfiguration("kompozr");
}

function mode(): RustAnalyzerMode {
    const value = settings().get<string>("rustAnalyzer.mode", "auto");
    return value === "kompozr" || value === "off" ? value : "auto";
}

function proxying(): boolean {
    return proxiesRust({ mode: mode(), rustAnalyzerExtension: vscode.extensions.getExtension(rustAnalyzerExtension) !== undefined });
}

let runningProxied = false;

/** A document that wants the server: started if it isn't. */
function check(document: vscode.TextDocument): void {
    if (client || document.uri.scheme !== "file") return;
    if (startsServer(document.languageId, () => document.getText(), proxying())) void start();
}

async function start(): Promise<void> {
    if (client) return;
    if (starting) return starting;
    starting = startClient().finally(() => {
        starting = undefined;
    });
    return starting;
}

async function startClient(): Promise<void> {
    const config = settings();
    const windows = process.platform === "win32";
    const folders = (vscode.workspace.workspaceFolders ?? []).map((f) => f.uri.fsPath);
    const command = findServer({ setting: config.get<string>("server.path", ""), env: process.env, folders, windows });
    const rustAnalyzerPath = config.get<string>("rustAnalyzer.path", "");
    // The shadow bridge's rust-analyzer when none is set and none is on PATH: the rust-analyzer extension's own.
    const extension = vscode.extensions.getExtension(rustAnalyzerExtension);
    const bundled = extension && !rustAnalyzerPath.trim() && !onPath("rust-analyzer", process.env, windows)
        ? path.join(extension.extensionPath, "server", windows ? "rust-analyzer.exe" : "rust-analyzer")
        : undefined;
    runningProxied = proxying();
    const args = serverArguments({
        mode: mode(),
        rustAnalyzerExtension: extension !== undefined,
        rustAnalyzerPath,
        shadow: config.get<boolean>("shadow.enabled", true),
        extraArgs: config.get<string[]>("server.extraArgs", []),
        bundledRustAnalyzer: bundled,
    });
    output.appendLine(`starting ${command} ${args.join(" ")}`);
    const server: ServerOptions = { command, args, options: { cwd: folders[0] } };
    const options: LanguageClientOptions = {
        documentSelector: [
            { scheme: "file", language: "rust" },
            { scheme: "file", language: "kotlin" },
        ],
        outputChannel: output,
    };
    const created = new LanguageClient("kompozr", "KompozR (kompoz-lsp)", server, options);
    created.onDidChangeState((e) => {
        if (e.newState === State.Stopped) output.appendLine("kompoz-lsp stopped");
    });
    client = created;
    try {
        await created.start();
    } catch (error) {
        if (client === created) client = undefined;
        const message = `KompozR: ${command} didn't start (${error instanceof Error ? error.message : error}).`;
        const choice = await vscode.window.showErrorMessage(message, "Settings");
        if (choice) void vscode.commands.executeCommand("workbench.action.openSettings", "kompozr.server.path");
    }
}

/** The server stopped and started again (`always`: even with no document asking for it yet). */
async function restart(always = false): Promise<void> {
    await starting;
    const old = client;
    client = undefined;
    await old?.stop();
    const proxied = proxying();
    if (always || vscode.workspace.textDocuments.some((d) => d.uri.scheme === "file" && startsServer(d.languageId, () => d.getText(), proxied))) {
        await start();
    }
}
