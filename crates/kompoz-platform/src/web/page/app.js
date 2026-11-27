// A KompozR app's page (kompoz-platform's web/page; `cargo xtask web` serves it with the app's module, app.wasm):
// the module instantiated with the web glue (kompoz.js), the WebGPU device's part (webgpu.js) and the web
// platform's (platform.js), the GPUDevice asked for first, then the app's `main`, which starts it in the canvas.
//
// With `KOMPOZ_WEB_REPORT=1` in the query (`cargo xtask web test`'s runs) what the app logs, the files it writes
// (a script's screenshots) and its end are sent to the server that served the page: /log, /file, /done. And
// `KOMPOZ_WEB_DRIVE=<frame>:<action>:<arguments>;…` acts on the page as a user would, before the animation frame of
// that number (the app's script's frame, while a script runs): `click:x,y` (a mouse's press and release on the
// canvas, CSS pixels), `key:code,key` (a key pressed and let go where the keyboard's focus is), `type:text` (text
// typed into the focused field's text area, as a phone's keyboard does: its input event), `aria:<file>` (the ARIA
// mirror as a screen reader reads it — each element's role, name, place on the canvas and states, indented by
// depth — written to the file, as the app's files are), `ariaclick:<role>:<name>` (the mirrored element of that role
// whose name begins so activated, as a screen reader does: its click).

import { instantiate } from "./kompoz.js";
import { gpuImports, requestGpu } from "./webgpu.js";
import { platformImports, runApp } from "./platform.js";

const status = document.getElementById("status");
const report = new URLSearchParams(location.search).get("KOMPOZ_WEB_REPORT") === "1";

let sending = Promise.resolve();
// Sent one after another (the server prints them as they come).
function send(path, body) {
    sending = sending.then(() => fetch(path, { method: "POST", body }).catch(() => {}));
    return sending;
}

function show(text) {
    if (status) {
        status.textContent = text;
        status.style.display = text ? "flex" : "none";
    }
}

async function start() {
    const { glue } = await instantiate(fetch("app.wasm"), {
        parts: [gpuImports, platformImports],
        before: [requestGpu],
    });
    if (report) {
        glue.onFrame = drive(glue, new URLSearchParams(location.search).get("KOMPOZ_WEB_DRIVE") || "");
        glue.onLog = (level, text) => send("/log", text);
        glue.onFile = (path, bytes) => send(`/file?path=${encodeURIComponent(path)}`, bytes);
        glue.onAlert = (message) => send("/log", `alert: ${message}`);
        glue.onEnded = (code) => send("/done", JSON.stringify({ code }));
    } else {
        glue.onEnded = (code) => show(code === 0 ? "" : `The app stopped (${code}): see the console.`);
    }
    show("");
    runApp(glue);
}

// The driver of `KOMPOZ_WEB_DRIVE`'s actions: a function of the frame's number.
function drive(glue, spec) {
    const actions = spec
        .split(";")
        .map((a) => a.trim())
        .filter(Boolean)
        .map((a) => {
            const [frame, kind, ...rest] = a.split(":");
            return { frame: Number(frame), kind, args: rest.join(":") };
        });
    return (frame) => {
        for (const a of actions.filter((a) => a.frame === frame)) {
            const canvas = glue.page.get("kompoz.canvas");
            const area = glue.platform.textArea;
            if (a.kind === "click") {
                const [x, y] = a.args.split(",").map(Number);
                const r = canvas.getBoundingClientRect();
                const at = { clientX: r.left + x, clientY: r.top + y, bubbles: true, pointerId: 1, pointerType: "mouse", isPrimary: true };
                canvas.dispatchEvent(new PointerEvent("pointerdown", { ...at, button: 0, buttons: 1, pressure: 0.5 }));
                canvas.dispatchEvent(new PointerEvent("pointerup", { ...at, button: 0, buttons: 0 }));
            } else if (a.kind === "key") {
                const [code, key] = a.args.split(",");
                const target = document.activeElement || canvas;
                for (const type of ["keydown", "keyup"]) target.dispatchEvent(new KeyboardEvent(type, { code, key, bubbles: true, cancelable: true }));
            } else if (a.kind === "aria") {
                send(`/file?path=${encodeURIComponent(a.args)}`, ariaDump(glue));
            } else if (a.kind === "ariaclick") {
                const [role, ...rest] = a.args.split(":");
                const name = rest.join(":");
                // (The nodes' elements, then the buttons of their custom actions.)
                const m = glue.platform.aria;
                const nodes = m ? [...m.nodes.values(), ...m.root.querySelectorAll("[data-kz-action]")] : [];
                const el = nodes.find((n) => n.getAttribute("role") === role && nameOf(n).startsWith(name));
                if (el) el.click();
                else send("/log", `ariaclick: no ${role} "${name}" in the mirror`);
            } else if (a.kind === "type") {
                const start = area.selectionStart;
                area.setRangeText(a.args, start, area.selectionEnd, "end");
                area.dispatchEvent(new InputEvent("input", { data: a.args, inputType: "insertText", bubbles: true }));
            }
        }
    };
}

// A mirrored element's name: its `aria-label`, else its own text.
function nameOf(el) {
    if (!el._kz) return el.getAttribute("aria-label") ?? el.textContent;
    return el.getAttribute("aria-label") ?? (el._kz.text ? el._kz.text.textContent : "");
}

// The ARIA mirror as a screen reader reads it: a line an element (its role, name, place and size on the canvas,
// its ARIA states, `focused` where the keyboard's focus is), its children indented under it; a linked field's text
// area (or password input) under its element; then what the live regions say.
function ariaDump(glue) {
    const m = glue.platform.aria;
    if (!m) return "(no mirror)\n";
    const canvas = glue.page.get("kompoz.canvas").getBoundingClientRect();
    const lines = [];
    const walk = (parent, depth) => {
        const indent = "  ".repeat(depth);
        for (const el of parent.children) {
            if (el._kzActions) {
                for (const b of el.children) lines.push(`${indent}button "${b.textContent}" (custom action)`);
                continue;
            }
            if (!el._kz) continue;
            const r = el.getBoundingClientRect();
            const states = [...el.attributes]
                .filter((a) => a.name.startsWith("aria-") && a.name !== "aria-label" && a.name !== "aria-owns")
                .map((a) => `${a.name}=${a.value}`);
            if (document.activeElement === el) states.push("focused");
            const place = `@${Math.round(r.left - canvas.left)},${Math.round(r.top - canvas.top)} ${Math.round(r.width)}x${Math.round(r.height)}`;
            lines.push(`${indent}${el.getAttribute("role") || "-"} "${nameOf(el)}" ${place}${states.length ? ` ${states.join(" ")}` : ""}`);
            const owned = el.getAttribute("aria-owns") && document.getElementById(el.getAttribute("aria-owns"));
            if (owned) {
                const value = owned.type === "password" ? "•".repeat(owned.value.length) : owned.value;
                const focused = document.activeElement === owned ? " focused" : "";
                const kind = owned.type === "password" ? "password input" : "text area";
                lines.push(`${indent}  textbox "${owned.getAttribute("aria-label") || ""}" value="${value}" (${kind})${focused}`);
            }
            walk(el, depth + 1);
        }
    };
    walk(m.root, 0);
    lines.push(`polite: ${m.polite.textContent}`, `assertive: ${m.assertive.textContent}`);
    return `${lines.join("\n")}\n`;
}

start().catch((e) => {
    const text = `The app couldn't start: ${(e && e.message) || e}`;
    show(text);
    console.error(e);
    if (report) send("/log", text).then(() => send("/done", JSON.stringify({ code: 2 })));
});
