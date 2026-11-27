// The web platform's calls (kompoz-platform's web/: sys.rs declares them, run.rs and window.rs make them), a part of
// the web glue (kompoz-web's js/kompoz.js), and the page's side of the app's loop:
//
// - The page's listeners only gather: each event a record (a line, its fields between tabs, strings
//   percent-encoded), taken by the module at its next turn (web_events). A turn is asked for (an animation frame;
//   a timeout while the page is hidden) as events come, as the app's waker asks, and as work the page did for the
//   app settles (a buffer read back, an image decoded: the glue's onSettled).
// - The module is called only between its own calls (`call`: never while Rust runs — a listener a call of Rust's
//   sets off, as focusing the text area does, only gathers). A Rust panic that escapes a call stops the app.
// - The canvas: the page's element of the id the module names, else one made to fill the page; its pixels its CSS
//   size times the device pixel ratio (its device-pixel content box where the browser tells it).
// - Typing goes into a hidden text area holding the focused field's text (the module keeps it so): what the
//   keyboard types, an input method composes, a phone's keyboard corrects, comes back as its input and composition
//   events. Keys that type are the text area's; the others (moving, editing, Enter, Tab, shortcuts) are the app's
//   alone, except Ctrl+C, Ctrl+X and Ctrl+V, whose copy, cut and paste events bring the clipboard: copy and cut call
//   the module at once (the app's copy goes into the event); Ctrl+V's key waits for its paste event's data.
// - Drops: a drop's strings and files read before the module is told (a file's bytes are read asynchronously).
//   Drags out: a drag the browser begins (`dragstart`) of a press the app's drag within it already took.
//
//   import { instantiate } from "./kompoz.js";
//   import { gpuImports, requestGpu } from "./webgpu.js";
//   import { platformImports, runApp } from "./platform.js";
//   const { glue } = await instantiate(fetch("app.wasm"), { parts: [gpuImports, platformImports], before: [requestGpu] });
//   runApp(glue);

const TEXT_TYPES = new Set(["text/plain", "text/html", "text/uri-list"]);
const APPLE = /Mac|iPhone|iPad|iPod/.test(navigator.platform || navigator.userAgent || "");

function encoded(s) {
    return encodeURIComponent(s);
}

// The modifiers held as the module reads them: Shift 1, Ctrl 2, Alt 4, Meta 8 (Command is Ctrl on Apple's systems;
// AltGr is neither Ctrl nor Alt: what it types isn't a shortcut).
function modifiers(e) {
    const altGr = e.getModifierState && e.getModifierState("AltGraph");
    let m = e.shiftKey ? 1 : 0;
    if (!altGr && (e.ctrlKey || (APPLE && e.metaKey))) m |= 2;
    if (!altGr && e.altKey) m |= 4;
    if (!APPLE && e.metaKey) m |= 8;
    return m;
}

class WebPlatform {
    constructor(glue) {
        this.glue = glue;
        this.queue = [];
        this.depth = 0;
        this.dead = false;
        this.ended = false;
        this.framePending = false;
        this.frames = 0;
        this.hiddenTimer = 0;
        this.wakeTimer = 0;
        this.canvas = null;
        this.textArea = null;
        this.fieldActive = false;
        this.devicePixels = [0, 0];
        this.refresh = 60;
        this.lastFrame = 0;
        this.clip = [];
        this.clipEvent = null;
        this.heldPaste = null;
        this.dragStart = null;
        this.ownDrag = false;
        this.images = new Map();
        this.nextImage = 1;
        this.devices = { microphone: false, camera: false };
        this.wakeLock = null;
        this.wantAwake = false;
        this.hash = location.hash;
        this.measureContext = null;
        this.drawCanvas = null;
        // Hooks a page sets (the test runner's): a file the app writes, an error it shows, its end, each animation
        // frame before the app's turn (its number from 1).
        glue.onFrame = glue.onFrame || null;
        glue.onFile = glue.onFile || null;
        glue.onAlert = glue.onAlert || null;
        glue.onEnded = glue.onEnded || null;
        glue.onSettled = () => this.schedule();
    }

    // ---- The module's turns ----

    // An export of the module, unless Rust runs now (then nothing: the caller only gathers).
    call(name, ...args) {
        if (this.depth > 0 || this.dead || !this.glue.instance) return 0;
        const f = this.glue.instance.exports[name];
        if (!f) return 0;
        this.depth++;
        try {
            return f(...args);
        } catch (e) {
            this.dead = true;
            this.glue.log(2, `kompoz: the app stopped: ${e}${e && e.stack ? `\n${e.stack}` : ""}`);
            this.end(101);
            return 0;
        } finally {
            this.depth--;
        }
    }

    record(...fields) {
        this.queue.push(fields.join("\t"));
        this.schedule();
    }

    // A turn soon: the next animation frame (a timeout while the page is hidden: the browser runs no frames then).
    schedule() {
        if (this.ended || this.dead) return;
        if (document.visibilityState === "visible") {
            this.requestFrame();
        } else if (!this.hiddenTimer) {
            this.hiddenTimer = setTimeout(() => {
                this.hiddenTimer = 0;
                this.call("kompoz_web_turn", 0, performance.now());
            }, 0);
        }
    }

    requestFrame() {
        if (this.framePending || this.ended || this.dead) return;
        this.framePending = true;
        requestAnimationFrame((time) => {
            this.framePending = false;
            this.measureRefresh(time);
            this.frames++;
            if (this.glue.onFrame) this.glue.onFrame(this.frames);
            this.call("kompoz_web_turn", 1, time);
        });
    }

    // The display's rate, from the frames' intervals (an average of the last ones).
    measureRefresh(time) {
        if (this.lastFrame) {
            const interval = time - this.lastFrame;
            if (interval > 3 && interval < 60) {
                const rate = 1000 / interval;
                this.refresh = this.refresh * 0.9 + rate * 0.1;
            }
        }
        this.lastFrame = time;
    }

    end(code) {
        if (this.ended) return;
        this.ended = true;
        clearTimeout(this.wakeTimer);
        clearTimeout(this.hiddenTimer);
        if (this.glue.onEnded) this.glue.onEnded(code);
    }

    // ---- The page ----

    start(canvasId) {
        if (typeof document === "undefined") return 0;
        const unavailable = this.glue.page.get("gpu.unavailable");
        if (unavailable) this.glue.log(2, `kompoz: no WebGPU: ${unavailable}`);
        let canvas = document.getElementById(canvasId);
        if (!canvas) {
            canvas = document.createElement("canvas");
            canvas.id = canvasId;
            document.body.appendChild(canvas);
        }
        this.canvas = canvas;
        Object.assign(canvas.style, {
            display: "block",
            touchAction: "none",
            outline: "none",
            userSelect: "none",
            webkitUserSelect: "none",
        });
        if (!canvas.style.width) this.fill(true);
        canvas.tabIndex = 0;
        canvas.draggable = true;
        this.glue.page.set("kompoz.canvas", canvas);

        const ta = document.createElement("textarea");
        this.textArea = ta;
        ta.setAttribute("aria-hidden", "true");
        ta.setAttribute("autocomplete", "off");
        ta.tabIndex = -1;
        Object.assign(ta.style, {
            position: "fixed",
            left: "0px",
            top: "0px",
            width: "1px",
            height: "1px",
            opacity: "0",
            fontSize: "16px", // (iOS zooms into a field with a smaller font)
            resize: "none",
            overflow: "hidden",
            border: "0",
            padding: "0",
            margin: "0",
            outline: "none",
            pointerEvents: "none",
            caretColor: "transparent",
            background: "transparent",
            color: "transparent",
            whiteSpace: "pre",
        });
        document.body.appendChild(ta);

        // The safe area's insets, as CSS has them.
        const probe = document.createElement("div");
        Object.assign(probe.style, {
            position: "fixed",
            visibility: "hidden",
            pointerEvents: "none",
            paddingLeft: "env(safe-area-inset-left, 0px)",
            paddingTop: "env(safe-area-inset-top, 0px)",
            paddingRight: "env(safe-area-inset-right, 0px)",
            paddingBottom: "env(safe-area-inset-bottom, 0px)",
        });
        document.body.appendChild(probe);
        this.safeProbe = probe;

        this.listen();
        this.listDevices();
        return 1;
    }

    // The canvas fills the page (or not: the app's size).
    fill(on) {
        const c = this.canvas;
        if (on) {
            document.documentElement.style.height = "100%";
            Object.assign(document.body.style, { margin: "0", height: "100%", overflow: "hidden" });
            Object.assign(c.style, { width: "100vw", height: "100vh" });
        }
    }

    state() {
        const c = this.canvas;
        const r = c.getBoundingClientRect();
        const m = (q) => (matchMedia(q).matches ? 1 : 0);
        const safe = getComputedStyle(this.safeProbe);
        const px = (v) => parseFloat(v) || 0;
        const hash = location.hash.length > 1 ? location.hash.substring(1) : "";
        let decodedHash = hash;
        try {
            decodedHash = decodeURIComponent(hash);
        } catch {}
        return [
            `width=${r.width}`,
            `height=${r.height}`,
            `ratio=${window.devicePixelRatio || 1}`,
            `device_width=${this.devicePixels[0]}`,
            `device_height=${this.devicePixels[1]}`,
            `dark=${m("(prefers-color-scheme: dark)")}`,
            `reduced_motion=${m("(prefers-reduced-motion: reduce)")}`,
            `fine=${m("(any-pointer: fine)")}`,
            `coarse=${m("(any-pointer: coarse)")}`,
            `focused=${document.hasFocus() ? 1 : 0}`,
            `visible=${document.visibilityState === "visible" ? 1 : 0}`,
            `fullscreen=${document.fullscreenElement ? 1 : 0}`,
            `hash=${encoded(decodedHash)}`,
            `safe=${px(safe.paddingLeft)},${px(safe.paddingTop)},${px(safe.paddingRight)},${px(safe.paddingBottom)}`,
            `screen=${screen.width},${screen.height}`,
            `microphone=${this.devices.microphone ? 1 : 0}`,
            `camera=${this.devices.camera ? 1 : 0}`,
            `refresh=${this.refresh}`,
        ].join("\n");
    }

    listDevices() {
        const md = navigator.mediaDevices;
        if (!md || !md.enumerateDevices) return;
        md.enumerateDevices()
            .then((list) => {
                this.devices = {
                    microphone: list.some((d) => d.kind === "audioinput"),
                    camera: list.some((d) => d.kind === "videoinput"),
                };
                this.record("page", "devices");
            })
            .catch(() => {});
    }

    listen() {
        const c = this.canvas;
        const ta = this.textArea;
        const at = (e) => {
            const r = c.getBoundingClientRect();
            return [e.clientX - r.left, e.clientY - r.top];
        };

        // The canvas's size, in device pixels where the browser tells them.
        if (typeof ResizeObserver === "function") {
            const observer = new ResizeObserver((entries) => {
                for (const entry of entries) {
                    const box = entry.devicePixelContentBoxSize && entry.devicePixelContentBoxSize[0];
                    this.devicePixels = box ? [box.inlineSize, box.blockSize] : [0, 0];
                }
                this.record("page", "resize");
            });
            try {
                observer.observe(c, { box: "device-pixel-content-box" });
            } catch {
                observer.observe(c);
            }
        }
        window.addEventListener("resize", () => this.record("page", "resize"));
        // The device pixel ratio's changes (zoom, another screen): a query for the ratio now, again each time.
        const watchRatio = () => {
            const q = matchMedia(`(resolution: ${window.devicePixelRatio}dppx)`);
            q.addEventListener("change", () => {
                this.record("page", "resize");
                watchRatio();
            }, { once: true });
        };
        watchRatio();
        for (const [query, what] of [
            ["(prefers-color-scheme: dark)", "theme"],
            ["(prefers-reduced-motion: reduce)", "motion"],
            ["(any-pointer: fine)", "pointer"],
            ["(any-pointer: coarse)", "pointer"],
        ]) {
            matchMedia(query).addEventListener("change", () => this.record("page", what));
        }
        window.addEventListener("focus", () => this.record("page", "focus"));
        window.addEventListener("blur", () => this.record("page", "focus"));
        document.addEventListener("visibilitychange", () => {
            const visible = document.visibilityState === "visible";
            this.record("page", visible ? "shown" : "hidden");
            if (visible && this.wantAwake) this.keepAwake(true);
            // (The state kept as the page hides: a phone may end a hidden tab without a pagehide.)
            if (!visible) this.call("kompoz_web_turn", 0, performance.now());
        });
        document.addEventListener("fullscreenchange", () => this.record("page", "fullscreen"));
        window.addEventListener("hashchange", () => {
            if (location.hash !== this.hash) {
                this.hash = location.hash;
                this.record("page", "hash");
            }
        });
        window.addEventListener("pagehide", () => this.call("kompoz_web_save"));
        if (navigator.mediaDevices && navigator.mediaDevices.addEventListener) {
            navigator.mediaDevices.addEventListener("devicechange", () => this.listDevices());
        }

        // Pointers: mice, pens, fingers.
        const pointer = (phase) => (e) => {
            const [x, y] = at(e);
            if (phase === "down" && e.pointerType !== "touch") {
                try {
                    c.setPointerCapture(e.pointerId);
                } catch {}
                if (!this.fieldActive) c.focus({ preventScroll: true });
            }
            this.record(
                "pointer",
                phase,
                e.pointerId,
                e.pointerType || "mouse",
                x,
                y,
                e.button,
                e.buttons,
                e.pressure,
                modifiers(e),
                e.timeStamp,
            );
        };
        c.addEventListener("pointerdown", pointer("down"));
        c.addEventListener("pointermove", (e) => {
            // (The moves the browser gathered between frames, each.)
            const all = e.getCoalescedEvents ? e.getCoalescedEvents() : [];
            if (all.length > 1) for (const one of all) pointer("move")(one);
            else pointer("move")(e);
        });
        c.addEventListener("pointerup", pointer("up"));
        c.addEventListener("pointercancel", pointer("cancel"));
        c.addEventListener("pointerleave", (e) => {
            if (e.pointerType === "touch" || (e.pointerType === "mouse" && e.buttons)) return;
            pointer("leave")(e);
        });
        // (A press keeps the focus on the text area while a field has it.)
        c.addEventListener("mousedown", (e) => {
            if (this.fieldActive) e.preventDefault();
        });
        c.addEventListener("contextmenu", (e) => e.preventDefault());
        c.addEventListener(
            "wheel",
            (e) => {
                e.preventDefault(); // (Ctrl+wheel, a touchpad's pinch, would zoom the page)
                const [x, y] = at(e);
                this.record("wheel", x, y, e.deltaX, e.deltaY, e.deltaMode, modifiers(e), e.timeStamp);
            },
            { passive: false },
        );

        // Keys.
        const ours = (e) => {
            const t = e.target;
            return t === ta || t === c || t === document.body || t === document.documentElement || t === window;
        };
        window.addEventListener("keydown", (e) => {
            if (!ours(e) || e.isComposing || e.keyCode === 229) return; // (an input method's)
            const m = modifiers(e);
            const rec = ["key", 1, encoded(e.code), encoded(e.key), m, e.timeStamp].join("\t");
            const shortcut = (m & (2 | 4 | 8)) !== 0;
            const lower = e.key.length === 1 ? e.key.toLowerCase() : "";
            const clipboard = (m & 2) !== 0 && !(m & 4) && (lower === "c" || lower === "x" || lower === "v");
            const types = !shortcut && ([...e.key].length === 1 || e.key === "Dead");
            if (clipboard && lower === "v" && !e.repeat) {
                // (Held until the paste event brought the clipboard, or 100 ms.)
                this.releasePaste();
                this.heldPaste = { rec, timer: setTimeout(() => this.releasePaste(), 100) };
                return;
            }
            this.queue.push(rec);
            this.schedule();
            if (!types && !clipboard) e.preventDefault();
        });
        window.addEventListener("keyup", (e) => {
            if (!ours(e) || e.isComposing || e.keyCode === 229) return;
            this.record("key", 0, encoded(e.code), encoded(e.key), modifiers(e), e.timeStamp);
        });

        // The text area: what is typed, composed, corrected.
        const sel = () => [ta.selectionStart, ta.selectionEnd];
        ta.addEventListener("input", () => this.record("input", encoded(ta.value), ...sel()));
        ta.addEventListener("compositionstart", () => this.record("compositionstart"));
        ta.addEventListener("compositionupdate", (e) =>
            this.record("compositionupdate", encoded(e.data || ""), encoded(ta.value), ...sel()),
        );
        ta.addEventListener("compositionend", (e) =>
            this.record("compositionend", encoded(e.data || ""), encoded(ta.value), ...sel()),
        );

        // The clipboard.
        const copy = (cut) => (e) => {
            if (!ours(e) && e.target !== ta) return;
            this.clipEvent = e;
            this.clipSet = false;
            this.call("kompoz_web_clipboard", cut ? 1 : 0);
            this.clipEvent = null;
            if (this.clipSet) e.preventDefault();
        };
        document.addEventListener("copy", copy(false));
        document.addEventListener("cut", copy(true));
        document.addEventListener("paste", (e) => {
            if (!ours(e) && e.target !== ta) return;
            e.preventDefault();
            const held = this.heldPaste;
            if (held) clearTimeout(held.timer);
            this.heldPaste = null;
            this.readTransfer(e.clipboardData).then((handle) => {
                if (handle) this.queue.push(`paste\t${handle}`);
                if (held) this.queue.push(held.rec);
                this.schedule();
            });
        });

        // Drops in, and the app's own drags out and back.
        const drag = (phase) => (e) => {
            e.preventDefault();
            const [x, y] = at(e);
            if (phase === "over") {
                e.dataTransfer.dropEffect = this.ownDrag && e.dataTransfer.effectAllowed.includes("ove") ? "move" : "copy";
                this.record("drag", "over", x, y, 0);
            } else if (phase === "enter") {
                if (this.dragInside) return;
                this.dragInside = true;
                const handle = this.ownDrag ? 0 : this.glue.add({ items: this.describe(e.dataTransfer) });
                this.record("drag", "enter", x, y, handle);
            } else if (phase === "leave") {
                if (e.relatedTarget && c.contains(e.relatedTarget)) return;
                this.dragInside = false;
                this.record("drag", "leave", x, y, 0);
            } else {
                this.dragInside = false;
                if (this.ownDrag) {
                    this.record("drag", "drop", x, y, 0);
                    return;
                }
                this.readTransfer(e.dataTransfer).then((handle) => this.record("drag", "drop", x, y, handle || 0));
            }
        };
        c.addEventListener("dragenter", drag("enter"));
        c.addEventListener("dragover", drag("over"));
        c.addEventListener("dragleave", drag("leave"));
        c.addEventListener("drop", drag("drop"));
        c.addEventListener("dragstart", (e) => {
            const [x, y] = at(e);
            this.dragStart = e;
            const taken = this.call("kompoz_web_drag_start", x, y);
            this.dragStart = null;
            if (!taken) {
                e.preventDefault();
                return;
            }
            this.ownDrag = true;
        });
        c.addEventListener("dragend", (e) => {
            if (!this.ownDrag) return;
            this.ownDrag = false;
            this.dragInside = false;
            this.record("dragend", e.dataTransfer ? e.dataTransfer.dropEffect : "none");
        });
    }

    releasePaste() {
        const held = this.heldPaste;
        if (!held) return;
        clearTimeout(held.timer);
        this.heldPaste = null;
        this.queue.push(held.rec);
        this.schedule();
    }

    // A transfer's items as its types say (before its drop: no data).
    describe(dt) {
        const items = [];
        for (const item of dt.items || []) items.push({ kind: item.kind, mime: item.type, name: "" });
        if (!items.length) for (const type of dt.types || []) items.push({ kind: "string", mime: type, name: "" });
        return items;
    }

    // A transfer's strings and files (their bytes read) kept in the glue's table: its handle (0: nothing).
    async readTransfer(dt) {
        if (!dt) return 0;
        const items = [];
        for (const type of dt.types || []) {
            if (type === "Files") continue;
            try {
                items.push({ kind: "string", mime: type, name: "", data: dt.getData(type) });
            } catch {}
        }
        const files = [...(dt.files || [])];
        const read = await Promise.all(
            files.map((f) =>
                f
                    .arrayBuffer()
                    .then((b) => ({ kind: "file", mime: f.type, name: f.name, data: new Uint8Array(b) }))
                    .catch(() => null),
            ),
        );
        for (const r of read) if (r) items.push(r);
        return items.length ? this.glue.add({ items }) : 0;
    }

    // ---- The text area ----

    textInput(active, x, y, w, h, kind) {
        const ta = this.textArea;
        const [mode, enter, capitalize, correct] = kind.split(" ");
        this.fieldActive = !!active;
        if (!active) {
            if (document.activeElement === ta) {
                ta.blur();
                this.canvas.focus({ preventScroll: true });
            }
            return;
        }
        const r = this.canvas.getBoundingClientRect();
        Object.assign(ta.style, {
            left: `${r.left + x}px`,
            top: `${r.top + y}px`,
            width: `${Math.max(1, w)}px`,
            height: `${Math.max(1, h)}px`,
        });
        ta.inputMode = mode || "text";
        ta.enterKeyHint = enter || "enter";
        ta.setAttribute("autocapitalize", capitalize || "off");
        ta.setAttribute("autocorrect", correct || "on");
        ta.spellcheck = correct === "on";
        if (document.activeElement !== ta) ta.focus({ preventScroll: true });
    }

    textSet(text, start, end) {
        const ta = this.textArea;
        if (ta.value !== text) ta.value = text;
        if (ta.selectionStart !== start || ta.selectionEnd !== end) ta.setSelectionRange(start, end);
    }

    // ---- The clipboard ----

    clipCommit(inEvent) {
        const decoder = new TextDecoder();
        const items = this.clip;
        this.clip = [];
        if (inEvent && this.clipEvent && this.clipEvent.clipboardData) {
            const data = this.clipEvent.clipboardData;
            for (const { mime, bytes } of items) {
                if (TEXT_TYPES.has(mime) || mime.startsWith("text/")) data.setData(mime, decoder.decode(bytes));
            }
            this.clipSet = true;
            // (An image goes through the Clipboard API: an event's data holds strings only.)
            const images = items.filter((i) => i.mime.startsWith("image/"));
            if (!images.length) return;
            this.clipWrite(images.concat(items.filter((i) => i.mime === "text/plain" || i.mime === "text/html")));
            return;
        }
        this.clipWrite(items);
    }

    clipWrite(items) {
        const cb = navigator.clipboard;
        if (!cb) return;
        const supported = items.filter(
            (i) => i.mime === "text/plain" || i.mime === "text/html" || i.mime === "image/png",
        );
        const write = supported.length && typeof ClipboardItem === "function" && cb.write
            ? cb.write([new ClipboardItem(Object.fromEntries(supported.map((i) => [i.mime, new Blob([i.bytes], { type: i.mime })])))])
            : cb.writeText
              ? cb.writeText("")
              : null;
        if (write) write.catch((e) => this.glue.log(1, `kompoz: the clipboard refused the copy: ${(e && e.message) || e}`));
    }

    // The app's drag handed to the browser's (only in a dragstart): what it carries, the actions it allows.
    dragOut(actions) {
        const e = this.dragStart;
        const items = this.clip;
        this.clip = [];
        if (!e || !e.dataTransfer) return 0;
        const dt = e.dataTransfer;
        const decoder = new TextDecoder();
        let label = "";
        for (const { mime, bytes } of items) {
            if (!mime.startsWith("text/")) continue;
            const text = decoder.decode(bytes);
            dt.setData(mime, text);
            if (mime === "text/plain") label = text;
        }
        const copy = actions & 1, move = actions & 2, link = actions & 4;
        dt.effectAllowed = copy && move && link ? "all" : copy && move ? "copyMove" : copy && link ? "copyLink"
            : move && link ? "linkMove" : move ? "move" : link ? "link" : "copy";
        // (The decoration drawn by the app isn't read back synchronously: a label of what it carries.)
        const image = document.createElement("canvas");
        const g = image.getContext("2d");
        g.font = "13px sans-serif";
        const text = label.length > 40 ? `${label.slice(0, 39)}…` : label || "⋯";
        const w = Math.ceil(g.measureText(text).width) + 16;
        image.width = w;
        image.height = 24;
        Object.assign(image.style, { position: "fixed", left: "-1000px", top: "0px" });
        g.fillStyle = "rgba(60,60,60,0.85)";
        g.fillRect(0, 0, w, 24);
        g.fillStyle = "#fff";
        g.font = "13px sans-serif";
        g.fillText(text, 8, 16);
        document.body.appendChild(image);
        dt.setDragImage(image, 0, 0);
        setTimeout(() => image.remove(), 0);
        return 1;
    }

    // ---- Text drawn by the browser ----

    measure(text, font) {
        if (!this.measureContext) {
            const canvas = typeof OffscreenCanvas === "function" ? new OffscreenCanvas(1, 1) : document.createElement("canvas");
            this.measureContext = canvas.getContext("2d");
        }
        const g = this.measureContext;
        g.font = font;
        const m = g.measureText(text);
        const ascent = m.fontBoundingBoxAscent ?? m.actualBoundingBoxAscent;
        const descent = m.fontBoundingBoxDescent ?? m.actualBoundingBoxDescent;
        return [
            m.width,
            m.actualBoundingBoxLeft,
            m.actualBoundingBoxRight,
            m.actualBoundingBoxAscent,
            m.actualBoundingBoxDescent,
            ascent,
            descent,
        ].join(" ");
    }

    draw(text, font, w, h, x, y) {
        if (!this.drawCanvas) {
            this.drawCanvas = typeof OffscreenCanvas === "function" ? new OffscreenCanvas(w, h) : document.createElement("canvas");
        }
        const canvas = this.drawCanvas;
        if (canvas.width < w) canvas.width = w;
        if (canvas.height < h) canvas.height = h;
        const g = canvas.getContext("2d", { willReadFrequently: true });
        g.clearRect(0, 0, w, h);
        g.font = font;
        g.textBaseline = "alphabetic";
        g.fillStyle = "#fff";
        g.fillText(text, x, y);
        return new Uint8Array(g.getImageData(0, 0, w, h).data.buffer);
    }

    // ---- Images from http(s) ----

    // `url` fetched and decoded by the browser, scaled down to fit `w` by `h` (to cover it with `fill`; 0: as it
    // is): its pixels, after the width and height, kept for the module (web_image_result).
    loadImage(url, w, h, fill) {
        const id = this.nextImage++;
        const entry = { done: false, bytes: null };
        this.images.set(id, entry);
        const work = (async () => {
            const response = await fetch(url);
            if (!response.ok) throw new Error(`HTTP ${response.status}`);
            const bitmap = await createImageBitmap(await response.blob());
            let [tw, th] = [bitmap.width, bitmap.height];
            if (w > 0 && h > 0) {
                const k = fill ? Math.max(w / tw, h / th) : Math.min(w / tw, h / th);
                if (k < 1) [tw, th] = [Math.max(1, Math.round(tw * k)), Math.max(1, Math.round(th * k))];
            }
            const canvas = typeof OffscreenCanvas === "function" ? new OffscreenCanvas(tw, th) : document.createElement("canvas");
            canvas.width = tw;
            canvas.height = th;
            const g = canvas.getContext("2d", { willReadFrequently: true });
            g.drawImage(bitmap, 0, 0, tw, th);
            bitmap.close();
            const pixels = g.getImageData(0, 0, tw, th).data;
            const bytes = new Uint8Array(8 + pixels.length);
            const head = new DataView(bytes.buffer);
            head.setUint32(0, tw, true);
            head.setUint32(4, th, true);
            bytes.set(pixels, 8);
            entry.bytes = bytes;
        })()
            .catch((e) => this.glue.log(1, `kompoz: can't load ${url}: ${(e && e.message) || e}`))
            .finally(() => {
                entry.done = true;
            });
        this.glue.track(work);
        return id;
    }

    imageResult(id) {
        const entry = this.images.get(id);
        if (!entry) return -1;
        if (!entry.done) return -2;
        this.images.delete(id);
        return this.glue.hold(entry.bytes);
    }

    // ---- The rest ----

    keepAwake(on) {
        this.wantAwake = !!on;
        if (!on) {
            if (this.wakeLock) this.wakeLock.release().catch(() => {});
            this.wakeLock = null;
            return;
        }
        if (!navigator.wakeLock || this.wakeLock) return;
        navigator.wakeLock
            .request("screen")
            .then((lock) => {
                this.wakeLock = lock;
                lock.addEventListener("release", () => {
                    if (this.wakeLock === lock) this.wakeLock = null;
                });
            })
            .catch(() => {});
    }

    storageKey(key) {
        return `kompoz:${location.pathname}:${key}`;
    }

    saveFile(path, bytes) {
        if (this.glue.onFile) return this.glue.onFile(path, bytes);
        const a = document.createElement("a");
        a.href = URL.createObjectURL(new Blob([bytes]));
        a.download = path.split(/[\\/]/).pop() || "file";
        a.click();
        setTimeout(() => URL.revokeObjectURL(a.href), 1000);
    }

    imports() {
        const glue = this.glue;
        const s = (pointer, length) => glue.string(pointer, length);
        const imports = {
            web_start: (id, length) => this.start(s(id, length)),
            web_state: () => glue.hold(this.state()),
            web_events: () => {
                const lines = this.queue.join("\n");
                this.queue = [];
                return glue.hold(lines);
            },
            web_request_frame: () => this.schedule(),
            web_wake_in: (ms) => {
                clearTimeout(this.wakeTimer);
                if (ms >= 0) this.wakeTimer = setTimeout(() => this.schedule(), ms);
            },
            web_ended: (code) => this.end(code),
            web_set_canvas_size: (w, h) => {
                if (w > 0 && h > 0) Object.assign(this.canvas.style, { width: `${w}px`, height: `${h}px` });
                else this.fill(true);
                this.record("page", "resize");
            },
            web_set_title: (p, l) => {
                document.title = s(p, l);
            },
            web_set_cursor: (p, l) => {
                this.canvas.style.cursor = s(p, l);
            },
            web_fullscreen: (on) => {
                const done = on
                    ? document.fullscreenElement || document.documentElement.requestFullscreen()
                    : document.fullscreenElement && document.exitFullscreen();
                if (done && done.catch) done.catch((e) => glue.log(1, `kompoz: full screen: ${(e && e.message) || e}`));
            },
            web_text_input: (active, x, y, w, h, p, l) => this.textInput(active, x, y, w, h, s(p, l)),
            web_text_set: (p, l, start, end) => this.textSet(s(p, l), start, end),
            web_clip_begin: () => {
                this.clip = [];
            },
            web_clip_add: (mp, ml, dp, dl) => {
                this.clip.push({ mime: s(mp, ml), bytes: glue.view(dp, dl).slice() });
            },
            web_clip_commit: (inEvent) => this.clipCommit(inEvent),
            web_transfer_items: (handle) => {
                const t = glue.get(handle);
                if (!t) return glue.hold("");
                return glue.hold(t.items.map((i) => `${i.kind}\t${i.mime}\t${encoded(i.name || "")}`).join("\n"));
            },
            web_transfer_data: (handle, index) => {
                const item = glue.get(handle)?.items[index];
                return item && item.data !== undefined ? glue.hold(item.data) : glue.hold(null);
            },
            web_drag_out: (actions) => this.dragOut(actions),
            web_open_uri: (p, l) => {
                window.open(s(p, l), "_blank", "noopener");
            },
            web_vibrate: (ms) => {
                if (navigator.vibrate) navigator.vibrate(ms);
            },
            web_keep_awake: (on) => this.keepAwake(on),
            web_location_set: (p, l) => {
                const h = `#${s(p, l)}`;
                this.hash = h;
                if (location.hash !== h) history.replaceState(history.state, "", h);
            },
            web_storage_get: (p, l) => {
                try {
                    const v = sessionStorage.getItem(this.storageKey(s(p, l)));
                    if (v === null) return glue.hold(null);
                    const binary = atob(v);
                    const bytes = new Uint8Array(binary.length);
                    for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
                    return glue.hold(bytes);
                } catch {
                    return glue.hold(null);
                }
            },
            web_storage_set: (kp, kl, dp, dl) => {
                const key = this.storageKey(s(kp, kl));
                try {
                    if (!dl) return sessionStorage.removeItem(key);
                    const bytes = glue.view(dp, dl);
                    let binary = "";
                    for (let i = 0; i < bytes.length; i += 0x8000) {
                        binary += String.fromCharCode.apply(null, bytes.subarray(i, i + 0x8000));
                    }
                    sessionStorage.setItem(key, btoa(binary));
                } catch (e) {
                    glue.log(1, `kompoz: can't keep the app's state: ${(e && e.message) || e}`);
                }
            },
            web_alert: (p, l) => {
                const message = s(p, l);
                if (glue.onAlert) glue.onAlert(message);
                else setTimeout(() => window.alert(message), 0);
            },
            web_file_save: (pp, pl, dp, dl) => this.saveFile(s(pp, pl), glue.view(dp, dl).slice()),
            web_text_measure: (tp, tl, fp, fl) => glue.hold(this.measure(s(tp, tl), s(fp, fl))),
            web_text_draw: (tp, tl, fp, fl, w, h, x, y) => glue.hold(this.draw(s(tp, tl), s(fp, fl), w, h, x, y)),
            web_image_load: (p, l, w, h, fill) => this.loadImage(s(p, l), w, h, fill),
            web_image_result: (id) => this.imageResult(id),
        };
        // What a call throws is logged, and the call returns 0 (none): a JavaScript exception must not unwind
        // through Rust's frames.
        for (const [name, call] of Object.entries(imports)) {
            imports[name] = (...args) => {
                try {
                    return call(...args);
                } catch (e) {
                    glue.log(2, `kompoz: web: ${name}: ${(e && e.message) || e}`);
                    return 0;
                }
            };
        }
        return imports;
    }
}

// The web platform's imports (a part of the glue's): its object kept on the glue (`glue.platform`).
export function platformImports(glue) {
    const platform = new WebPlatform(glue);
    glue.platform = platform;
    return platform.imports();
}

// The app run: its `main` (which starts it in the canvas and returns; the page runs it on from there).
export function runApp(glue) {
    const status = glue.platform.call("main", 0, 0);
    if (status !== 0 && !glue.platform.ended) {
        glue.log(2, `kompoz: the app didn't start (${status})`);
        glue.platform.end(status || 1);
    }
}
