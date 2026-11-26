// kompoz-lsp, the server the extension starts: where it is, the arguments it is given, and which documents start
// it. No `vscode` here (the unit tests run it under plain node); extension.ts reads the settings and passes them in.

import * as fs from "fs";
import * as path from "path";

/** Who answers for the Rust in .rs files (the `kompozr.rustAnalyzer.mode` setting). */
export type RustAnalyzerMode = "auto" | "kompozr" | "off";

export interface Where {
    setting: string;          // kompozr.server.path
    env: NodeJS.ProcessEnv;
    folders: string[];        // the workspace folders' paths
    windows: boolean;
}

/**
 * Where kompoz-lsp is: the setting; else KOMPOZ_LSP from the environment; else one built in a workspace folder's
 * target directory (target/debug or target/release, and CARGO_TARGET_DIR's, the newest); else kompoz-lsp on PATH.
 */
export function findServer(where: Where): string {
    if (where.setting.trim()) return where.setting.trim();
    const fromEnv = where.env.KOMPOZ_LSP?.trim();
    if (fromEnv) return fromEnv;
    const name = where.windows ? "kompoz-lsp.exe" : "kompoz-lsp";
    const targets = where.folders.map((folder) => path.join(folder, "target"));
    const cargoTarget = where.env.CARGO_TARGET_DIR?.trim();
    if (cargoTarget) {
        if (path.isAbsolute(cargoTarget)) targets.push(cargoTarget);
        else targets.push(...where.folders.map((folder) => path.join(folder, cargoTarget)));
    }
    let newest: { file: string; time: number } | undefined;
    for (const target of targets) {
        for (const profile of ["debug", "release"]) {
            const file = path.join(target, profile, name);
            try {
                fs.accessSync(file, fs.constants.X_OK);
                const stat = fs.statSync(file);
                if (stat.isFile() && (!newest || stat.mtimeMs > newest.time)) newest = { file, time: stat.mtimeMs };
            } catch {
                // (not built there)
            }
        }
    }
    return newest?.file ?? name;
}

export interface Options {
    mode: RustAnalyzerMode;
    rustAnalyzerExtension: boolean;   // rust-lang.rust-analyzer installed
    rustAnalyzerPath: string;         // kompozr.rustAnalyzer.path
    shadow: boolean;                  // kompozr.shadow.enabled
    extraArgs: string[];              // kompozr.server.extraArgs
    /** The rust-analyzer extension's own server, for the shadow bridge when no rust-analyzer is set. */
    bundledRustAnalyzer?: string;
}

/** Whether kompoz-lsp runs rust-analyzer for the Rust (else the Kotlin's answers only). */
export function proxiesRust(options: Pick<Options, "mode" | "rustAnalyzerExtension">): boolean {
    switch (options.mode) {
        case "kompozr":
            return true;
        case "off":
            return false;
        default:
            return !options.rustAnalyzerExtension;
    }
}

/**
 * kompoz-lsp's arguments: --lsp; the rust-analyzer it runs (--rust-analyzer PATH) or none (--no-rust-analyzer); the
 * shadow-project bridge's rust-analyzer (--shadow-rust-analyzer PATH: the one set, else the rust-analyzer extension's
 * own server when it is installed) or no bridge (--no-shadow); then the extra
 * arguments, rust-analyzer's own, passed on.
 */
export function serverArguments(options: Options): string[] {
    const args = ["--lsp"];
    const rustAnalyzer = options.rustAnalyzerPath.trim();
    if (!proxiesRust(options)) args.push("--no-rust-analyzer");
    else if (rustAnalyzer) args.push("--rust-analyzer", rustAnalyzer);
    const shadowAnalyzer = rustAnalyzer || options.bundledRustAnalyzer?.trim();
    if (!options.shadow) args.push("--no-shadow");
    else if (shadowAnalyzer) args.push("--shadow-rust-analyzer", shadowAnalyzer);
    return args.concat(options.extraArgs);
}

/** Whether a Rust source has KompozR's Kotlin in it (a kompoz! block, a kompoz_file!). */
export function hasKompoz(text: string): boolean {
    return /\bkompoz(?:_file)?!/.test(text);
}

/**
 * Whether an opened document starts the server: a .kt file; a Rust source with kompoz! in it; any Rust source when
 * kompoz-lsp is the one in front of rust-analyzer (there is no other server for the Rust then).
 */
export function startsServer(languageId: string, text: () => string, proxying: boolean): boolean {
    if (languageId === "kotlin") return true;
    if (languageId !== "rust") return false;
    return proxying || hasKompoz(text());
}

/** The file a command of this name runs from PATH, if there is one. */
export function onPath(name: string, env: NodeJS.ProcessEnv, windows: boolean): string | undefined {
    const dirs = (env.PATH ?? env.Path ?? "").split(windows ? ";" : ":").filter((d) => d);
    const names = windows
        ? (env.PATHEXT ?? ".EXE;.CMD;.BAT").split(";").filter((e) => e).map((ext) => name + ext.toLowerCase()).concat(name)
        : [name];
    for (const dir of dirs) {
        for (const candidate of names) {
            const file = path.join(dir, candidate);
            try {
                fs.accessSync(file, fs.constants.X_OK);
                if (fs.statSync(file).isFile()) return file;
            } catch {
                // (not there)
            }
        }
    }
    return undefined;
}
