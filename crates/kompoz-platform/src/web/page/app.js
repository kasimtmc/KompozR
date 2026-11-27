// A KompozR app's page (kompoz-platform's web/page; `cargo xtask web` serves it with the app's module, app.wasm):
// the module instantiated with the web glue (kompoz.js), the WebGPU device's part (webgpu.js) and the web
// platform's (platform.js), the GPUDevice asked for first, then the app's `main`, which starts it in the canvas.
//
// With `KOMPOZ_WEB_REPORT=1` in the query (`cargo xtask web test`'s runs) what the app logs, the files it writes
// (a script's screenshots) and its end are sent to the server that served the page: /log, /file, /done. And
// `KOMPOZ_WEB_DRIVE=<frame>:<action>:<arguments>;…` acts on the page as a user would, before the animation frame of
// that number (the app's script's frame, while a script runs): `click:x,y` (a mouse's press and release on the
// canvas, CSS pixels), `key:code,key` (a key pressed and let go where the keyboard's focus is), `type:text` (text
// typed into the focused field's text area, as a phone's keyboard does: its input event).

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
            } else if (a.kind === "type") {
                const start = area.selectionStart;
                area.setRangeText(a.args, start, area.selectionEnd, "end");
                area.dispatchEvent(new InputEvent("input", { data: a.args, inputType: "insertText", bubbles: true }));
            }
        }
    };
}

start().catch((e) => {
    const text = `The app couldn't start: ${(e && e.message) || e}`;
    show(text);
    console.error(e);
    if (report) send("/log", text).then(() => send("/done", JSON.stringify({ code: 2 })));
});
