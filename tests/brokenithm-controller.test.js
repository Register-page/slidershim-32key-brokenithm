const assert = require("assert");
const fs = require("fs");
const vm = require("vm");

const root = "src-slider_io/src/device/brokenithm-www";
const expectedTop = Array.from({ length: 16 }, (_, index) => index * 2 + 1);
const expectedBottom = Array.from({ length: 16 }, (_, index) => index * 2);

function readGroundRows(filename) {
  const html = fs.readFileSync(`${root}/${filename}`, "utf8");
  const start = html.indexOf('<div class="touch-container touch-grid">');
  assert.notStrictEqual(start, -1, `${filename} must contain the 2x16 grid`);
  const flags = [...html.slice(start).matchAll(/data-kflag="(\d+)"/g)]
    .slice(0, 32)
    .map((match) => Number(match[1]));
  return [flags.slice(0, 16), flags.slice(16)];
}

for (const filename of ["index.html", "index-go.html"]) {
  const [top, bottom] = readGroundRows(filename);
  assert.deepStrictEqual(top, expectedTop, `${filename}: top row`);
  assert.deepStrictEqual(bottom, expectedBottom, `${filename}: bottom row`);
  assert.deepStrictEqual(
    [...top, ...bottom].sort((a, b) => a - b),
    Array.from({ length: 32 }, (_, index) => index),
    `${filename}: every ground bit must appear exactly once`
  );
}

class Key {
  constructor(flag, air, left, top, width, height) {
    this.dataset = { kflag: String(flag), air: air ? "1" : "0" };
    this.offsetLeft = left;
    this.offsetTop = top;
    this.offsetWidth = width;
    this.offsetHeight = height;
    this.previousElementSibling = null;
    this.nextElementSibling = null;
  }
  setAttribute() {}
  removeAttribute() {}
}

function link(keys) {
  keys.forEach((key, index) => {
    key.previousElementSibling = keys[index - 1] || null;
    key.nextElementSibling = keys[index + 1] || null;
  });
  return keys;
}

function createRuntime(invert) {
  const groundTop = invert ? 0 : 384;
  const airTop = invert ? 384 : 0;
  const air = link(
    [5, 4, 3, 2, 1, 0].map(
      (flag, index) => new Key(flag, true, 0, airTop + index * 64, 1024, 64)
    )
  );
  const top = link(
    expectedTop.map(
      (flag, index) => new Key(flag, false, index * 64, groundTop, 64, 192)
    )
  );
  const bottom = link(
    expectedBottom.map(
      (flag, index) =>
        new Key(flag, false, index * 64, groundTop + 192, 64, 192)
    )
  );
  const listeners = {};
  const sent = [];
  const styles = [];
  const canvasContext = {
    getImageData: () => ({ data: new Uint8ClampedArray(132) }),
    putImageData: () => {},
  };
  const elements = {
    canvas: { getContext: () => canvasContext },
    fullscreen: { requestFullscreen: () => {} },
    main: {
      addEventListener: (name, handler) => {
        listeners[name] = handler;
      },
    },
  };

  class WebSocket {
    send(message) {
      sent.push(message);
    }
    close() {}
  }

  const context = {
    Uint8Array,
    Uint8ClampedArray,
    WebSocket,
    alert: (error) => {
      throw error;
    },
    config: {
      invert,
      bgColor: "",
      keyColor: "#ff00ff",
      lkeyColor: "#00ffff",
      keyHeight: 2,
      lkeyHeight: 3,
    },
    document: {
      fullscreenElement: null,
      getElementsByClassName: () => [...air, ...top, ...bottom],
      getElementById: (id) => elements[id],
      createElement: () => ({ innerHTML: "" }),
      head: { appendChild: (style) => styles.push(style.innerHTML) },
    },
    location: { protocol: "http:", host: "localhost:1606" },
    screen: { height: 768 },
    setInterval: () => 0,
    setTimeout: (callback) => {
      callback();
      return 0;
    },
    window: { allAir: false, outerWidth: 1024, outerHeight: 768 },
  };

  vm.createContext(context);
  vm.runInContext(fs.readFileSync(`${root}/app.js`, "utf8"), context);
  context.ws.onmessage({ data: "alive" });
  assert.match(styles[0], /rgba\(0, 0, 0, 0\.9\)/);
  assert.match(styles[0], /\.key\.air\[data-active\].*#00ffff/);
  assert.match(styles[0], /\.air-container \{flex: 3;\}/);

  const frameFor = (touches) => {
    sent.length = 0;
    listeners.touchstart({
      preventDefault: () => {},
      touches: touches.map(([clientX, clientY]) => ({ clientX, clientY })),
    });
    assert.strictEqual(sent.length, 1, "one changed input frame expected");
    assert.strictEqual(sent[0].length, 39, "frame must be b + 38 bits");
    return sent[0].slice(1).split("").map(Number);
  };

  frameFor.repeat = (touches) => {
    sent.length = 0;
    const event = {
      preventDefault: () => {},
      touches: touches.map(([clientX, clientY]) => ({ clientX, clientY })),
    };
    listeners.touchstart(event);
    listeners.touchstart(event);
    assert.strictEqual(sent.length, 1, "unchanged input must not be resent");
  };

  return frameFor;
}

function verifyRuntime(invert) {
  const frameFor = createRuntime(invert);
  const topY = invert ? 96 : 480;
  const bottomY = invert ? 288 : 672;

  let frame = frameFor([[32, topY]]);
  assert.strictEqual(frame[1], 1, "top-left must set bit 1");

  frame = frameFor([[32, bottomY]]);
  assert.strictEqual(frame[0], 1, "bottom-left must set bit 0");

  frame = frameFor([
    [32, topY],
    [32, bottomY],
    [96, topY],
    [96, bottomY],
  ]);
  assert.deepStrictEqual(frame.slice(0, 4), [1, 1, 1, 1]);

  frame = frameFor([
    [32, topY],
    [32, topY],
  ]);
  assert.strictEqual(
    frame.reduce((sum, bit) => sum + bit, 0),
    1,
    "duplicate touches must not spill into an adjacent bit"
  );

  createRuntime(invert).repeat([[32, topY]]);
}

verifyRuntime(false);
verifyRuntime(true);
console.log("Brokenithm 2x16 controller tests passed");
