import { test } from "node:test";
import * as assert from "node:assert/strict";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import { findServer, hasKompoz, onPath, Options, proxiesRust, serverArguments, startsServer } from "../src/server";

function scratch(): string {
    return fs.mkdtempSync(path.join(os.tmpdir(), "kompozr-vscode-"));
}

function built(file: string, seconds: number): string {
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, "#!/bin/sh\n", { mode: 0o755 });
    fs.utimesSync(file, seconds, seconds);
    return file;
}

test("kompoz-lsp is the setting's, else KOMPOZ_LSP's, else the newest built in a workspace target, else on PATH", () => {
    const root = scratch();
    try {
        const where = { setting: "", env: {}, folders: [root], windows: false };
        assert.equal(findServer(where), "kompoz-lsp");
        assert.equal(findServer({ ...where, windows: true }), "kompoz-lsp.exe");
        const debug = built(path.join(root, "target", "debug", "kompoz-lsp"), 1000);
        assert.equal(findServer(where), debug);
        const release = built(path.join(root, "target", "release", "kompoz-lsp"), 2000);
        assert.equal(findServer(where), release);
        fs.utimesSync(debug, 3000, 3000);
        assert.equal(findServer(where), debug);
        // (not a profile directory, and not executable)
        built(path.join(root, "target", "other", "kompoz-lsp"), 9000);
        assert.equal(findServer(where), debug);
        fs.chmodSync(debug, 0o644);
        assert.equal(findServer(where), release);
        assert.equal(findServer({ ...where, env: { KOMPOZ_LSP: "/opt/kompoz-lsp" } }), "/opt/kompoz-lsp");
        assert.equal(findServer({ ...where, setting: " /x/kompoz-lsp ", env: { KOMPOZ_LSP: "/opt/kompoz-lsp" } }), "/x/kompoz-lsp");
    } finally {
        fs.rmSync(root, { recursive: true, force: true });
    }
});

test("a CARGO_TARGET_DIR's build is found too, absolute or the workspace's", () => {
    const root = scratch();
    try {
        const elsewhere = built(path.join(root, "shared-target", "debug", "kompoz-lsp"), 1000);
        const folder = path.join(root, "ws");
        fs.mkdirSync(folder);
        const where = { setting: "", folders: [folder], windows: false };
        assert.equal(findServer({ ...where, env: { CARGO_TARGET_DIR: path.join(root, "shared-target") } }), elsewhere);
        const relative = built(path.join(folder, "out", "release", "kompoz-lsp"), 2000);
        assert.equal(findServer({ ...where, env: { CARGO_TARGET_DIR: "out" } }), relative);
    } finally {
        fs.rmSync(root, { recursive: true, force: true });
    }
});

const base: Options = { mode: "auto", rustAnalyzerExtension: false, rustAnalyzerPath: "", shadow: true, extraArgs: [] };

test("the auto mode proxies rust-analyzer unless its extension is installed", () => {
    assert.ok(proxiesRust({ mode: "auto", rustAnalyzerExtension: false }));
    assert.ok(!proxiesRust({ mode: "auto", rustAnalyzerExtension: true }));
    assert.ok(proxiesRust({ mode: "kompozr", rustAnalyzerExtension: true }));
    assert.ok(!proxiesRust({ mode: "off", rustAnalyzerExtension: false }));
});

test("the server is started with --lsp, the rust-analyzer it runs or none, the shadow bridge's, then the extra arguments", () => {
    assert.deepEqual(serverArguments(base), ["--lsp"]);
    assert.deepEqual(serverArguments({ ...base, rustAnalyzerExtension: true }), ["--lsp", "--no-rust-analyzer"]);
    assert.deepEqual(serverArguments({ ...base, mode: "kompozr", rustAnalyzerExtension: true }), ["--lsp"]);
    assert.deepEqual(serverArguments({ ...base, mode: "off" }), ["--lsp", "--no-rust-analyzer"]);
    assert.deepEqual(serverArguments({ ...base, rustAnalyzerPath: " /ra " }), [
        "--lsp", "--rust-analyzer", "/ra", "--shadow-rust-analyzer", "/ra",
    ]);
    // (no rust-analyzer in front, but the shadow bridge still runs one)
    assert.deepEqual(serverArguments({ ...base, mode: "off", rustAnalyzerPath: "/ra" }), [
        "--lsp", "--no-rust-analyzer", "--shadow-rust-analyzer", "/ra",
    ]);
    assert.deepEqual(serverArguments({ ...base, shadow: false, rustAnalyzerPath: "/ra" }), ["--lsp", "--rust-analyzer", "/ra", "--no-shadow"]);
    assert.deepEqual(serverArguments({ ...base, mode: "off", shadow: false }), ["--lsp", "--no-rust-analyzer", "--no-shadow"]);
    assert.deepEqual(serverArguments({ ...base, extraArgs: ["--log-file", "/tmp/ra.log"] }), ["--lsp", "--log-file", "/tmp/ra.log"]);
});

test("the rust-analyzer extension's own server runs the shadow bridge only when none is set", () => {
    const withExtension = { ...base, rustAnalyzerExtension: true, bundledRustAnalyzer: "/ext/server/rust-analyzer" };
    assert.deepEqual(serverArguments(withExtension), ["--lsp", "--no-rust-analyzer", "--shadow-rust-analyzer", "/ext/server/rust-analyzer"]);
    assert.deepEqual(serverArguments({ ...withExtension, rustAnalyzerPath: "/ra" }), [
        "--lsp", "--no-rust-analyzer", "--shadow-rust-analyzer", "/ra",
    ]);
    assert.deepEqual(serverArguments({ ...withExtension, shadow: false }), ["--lsp", "--no-rust-analyzer", "--no-shadow"]);
});

test("Rust sources with kompoz! are told from plain Rust", () => {
    assert.ok(hasKompoz("kompoz! {\n    fun A() {}\n}\n"));
    assert.ok(hasKompoz("let page = kompoz!(Text(\"a\"));"));
    assert.ok(hasKompoz("kompoz::kompoz_file!(\"src/main.kt\");"));
    assert.ok(!hasKompoz("fn main() { println!(\"kompoz\"); }"));
    assert.ok(!hasKompoz("my_kompoz! { }"));
});

test("a .kt file starts the server; a Rust source does with kompoz! in it, or always while proxying rust-analyzer", () => {
    const plain = () => "fn main() {}";
    const kompoz = () => "kompoz! { }";
    assert.ok(startsServer("kotlin", plain, false));
    assert.ok(startsServer("rust", kompoz, false));
    assert.ok(!startsServer("rust", plain, false));
    assert.ok(startsServer("rust", plain, true));
    assert.ok(!startsServer("cpp", kompoz, true));
});

test("a command is found on PATH when it is an executable file there", () => {
    const root = scratch();
    try {
        const dir = path.join(root, "bin");
        const ra = built(path.join(dir, "rust-analyzer"), 1000);
        assert.equal(onPath("rust-analyzer", { PATH: `${path.join(root, "none")}:${dir}` }, false), ra);
        assert.equal(onPath("kompoz-lsp", { PATH: dir }, false), undefined);
        assert.equal(onPath("rust-analyzer", {}, false), undefined);
    } finally {
        fs.rmSync(root, { recursive: true, force: true });
    }
});
