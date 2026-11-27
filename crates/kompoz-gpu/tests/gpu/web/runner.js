// The GPU tests' page (tests/gpu/web.rs is the module's side; `cargo xtask web-gpu-test` serves this and reads
// what it sends): the test binary built for the web (gpu.wasm) instantiated with the web glue (kompoz.js), the
// WebGPU device's part (webgpu.js) and the tests' own imports, then its `main` run. What the run prints, the
// pictures it writes and its result are sent to the server, in order (/log, /file, /done).
//
// The tests read pixels back at once, as natively; WebGPU maps buffers asynchronously, so `main` runs as a
// promising function (JSPI) and test_wait_for_gpu suspends it until the GPU work the glue tracks has settled.
// Only the tests do this: apps read back asynchronously (Device::read_pixels_async).

import { instantiate } from "./kompoz.js";
import { gpuImports, requestGpu } from "./webgpu.js";

const out = document.getElementById("out");
let sending = Promise.resolve();
// Sent one after another (the server prints them as they come).
function send(path, body) {
    sending = sending.then(() => fetch(path, { method: "POST", body }).catch(() => {}));
    return sending;
}
function say(text) {
    out.textContent += `\n${text}`;
    send("/log", text);
}

let finished = null;

function testImports(glue) {
    return {
        test_wait_for_gpu: new WebAssembly.Suspending(async () => {
            while (glue.pending.size) await Promise.allSettled([...glue.pending]);
        }),
        test_save_file: (path, pathLength, bytes, length) => {
            const name = glue.string(path, pathLength);
            send(`/file?path=${encodeURIComponent(name)}`, glue.view(bytes, length).slice());
        },
        test_finish: (checks, failures) => {
            finished = { checks, failures };
        },
    };
}

async function run() {
    out.textContent = "";
    if (typeof WebAssembly.Suspending !== "function" || typeof WebAssembly.promising !== "function") {
        say("this browser has no JavaScript Promise Integration (WebAssembly.Suspending), which the tests wait with");
        return { checks: 0, failures: 1 };
    }
    const { glue, instance } = await instantiate(fetch("gpu.wasm"), {
        parts: [gpuImports, testImports],
        before: [requestGpu],
    });
    glue.onLog = (level, text) => say(text);
    try {
        const status = await WebAssembly.promising(instance.exports.main)(0, 0);
        if (!finished) say(`main returned ${status} before the run finished`);
    } catch (e) {
        say(`the run stopped: ${e}${e && e.stack ? `\n${e.stack}` : ""}`);
        return { checks: finished?.checks ?? 0, failures: (finished?.failures ?? 0) + 1 };
    }
    return finished ?? { checks: 0, failures: 1 };
}

run()
    .catch((e) => {
        say(`the page failed: ${e}${e && e.stack ? `\n${e.stack}` : ""}`);
        return { checks: 0, failures: 1 };
    })
    .then((result) => send("/done", JSON.stringify(result)));
