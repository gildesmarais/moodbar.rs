const { requireNativeModule } = require("expo-modules-core");

const Native = requireNativeModule("MoodbarNative");
const finalizer =
  typeof FinalizationRegistry === "function"
    ? new FinalizationRegistry((handle) => {
        Native.disposeAnalysis(handle).catch(() => {});
      })
    : null;

class NativeAnalysis {
  constructor(handle, frameCount, channelCount) {
    this.__nativeHandle = handle;
    this.frameCount = frameCount;
    this.channelCount = channelCount;
    this.disposed = false;

    if (finalizer) {
      finalizer.register(this, handle, this);
    }
  }

  async dispose() {
    if (this.disposed) {
      return;
    }
    await Native.disposeAnalysis(this.__nativeHandle);
    this.disposed = true;
    if (finalizer) {
      finalizer.unregister(this);
    }
  }
}

function assertAnalysis(analysis) {
  if (!(analysis instanceof NativeAnalysis)) {
    throw new TypeError("analysis must be returned by analyze() from @moodbar/native");
  }
  if (analysis.disposed) {
    throw new Error("analysis handle has already been disposed");
  }
}

function stringifyOptions(options) {
  if (!options) {
    return "{}";
  }
  return JSON.stringify(options);
}

function inferExtension(name, mimeType) {
  if (typeof name === "string") {
    const match = name.match(/\.([A-Za-z0-9]+)$/);
    if (match) {
      return match[1].toLowerCase();
    }
  }

  if (typeof mimeType === "string") {
    const slash = mimeType.indexOf("/");
    if (slash >= 0 && slash < mimeType.length - 1) {
      return mimeType.slice(slash + 1).toLowerCase();
    }
  }

  return null;
}

const BASE64_ALPHABET =
  "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const BASE64_REVERSE = (() => {
  const out = new Uint8Array(256);
  out.fill(255);
  for (let i = 0; i < BASE64_ALPHABET.length; i += 1) {
    out[BASE64_ALPHABET.charCodeAt(i)] = i;
  }
  out["=".charCodeAt(0)] = 0;
  return out;
})();

function bytesToBase64(bytes) {
  let out = "";
  for (let i = 0; i < bytes.length; i += 3) {
    const a = bytes[i];
    const b = i + 1 < bytes.length ? bytes[i + 1] : 0;
    const c = i + 2 < bytes.length ? bytes[i + 2] : 0;
    const triple = (a << 16) | (b << 8) | c;

    out += BASE64_ALPHABET[(triple >> 18) & 63];
    out += BASE64_ALPHABET[(triple >> 12) & 63];
    out += i + 1 < bytes.length ? BASE64_ALPHABET[(triple >> 6) & 63] : "=";
    out += i + 2 < bytes.length ? BASE64_ALPHABET[triple & 63] : "=";
  }
  return out;
}

function base64ToBytes(base64) {
  const clean = base64.replace(/\s+/g, "");
  if (clean.length % 4 !== 0) {
    throw new Error("invalid base64 payload from native renderPng");
  }

  const padding = clean.endsWith("==") ? 2 : clean.endsWith("=") ? 1 : 0;
  const outLen = (clean.length / 4) * 3 - padding;
  const out = new Uint8Array(outLen);
  let outIdx = 0;

  for (let i = 0; i < clean.length; i += 4) {
    const c0 = BASE64_REVERSE[clean.charCodeAt(i)];
    const c1 = BASE64_REVERSE[clean.charCodeAt(i + 1)];
    const c2 = BASE64_REVERSE[clean.charCodeAt(i + 2)];
    const c3 = BASE64_REVERSE[clean.charCodeAt(i + 3)];
    if (c0 === 255 || c1 === 255 || c2 === 255 || c3 === 255) {
      throw new Error("invalid base64 payload from native renderPng");
    }

    const triple = (c0 << 18) | (c1 << 12) | (c2 << 6) | c3;
    if (outIdx < outLen) out[outIdx++] = (triple >> 16) & 255;
    if (outIdx < outLen) out[outIdx++] = (triple >> 8) & 255;
    if (outIdx < outLen) out[outIdx++] = triple & 255;
  }

  return out;
}

async function analyze(input, options) {
  const optionsJson = stringifyOptions(options);

  if (input && typeof input.uri === "string") {
    const result = await Native.analyzeFromUri(input.uri, optionsJson);
    return new NativeAnalysis(result.handle, result.frameCount, result.channelCount);
  }

  if (input && input.bytes instanceof Uint8Array) {
    const extension = inferExtension(input.name, input.mimeType);
    const base64 = bytesToBase64(input.bytes);
    const result = await Native.analyzeFromBase64(base64, extension, optionsJson);
    return new NativeAnalysis(result.handle, result.frameCount, result.channelCount);
  }

  throw new TypeError("input must be { uri: string } or { bytes: Uint8Array }");
}

async function render(analysis, format, options) {
  assertAnalysis(analysis);

  const optionsJson = stringifyOptions(options);
  if (format === "png") {
    const pngBase64 = await Native.renderPng(analysis.__nativeHandle, optionsJson);
    return base64ToBytes(pngBase64);
  }
  if (format === "svg") {
    return Native.renderSvg(analysis.__nativeHandle, optionsJson);
  }
  throw new TypeError('format must be "png" or "svg"');
}

async function generate(input, format, options) {
  const analysis = await analyze(input, options);
  try {
    return await render(analysis, format, options);
  } finally {
    await analysis.dispose();
  }
}

async function disposeAnalysis(analysis) {
  assertAnalysis(analysis);
  await analysis.dispose();
}

module.exports = {
  analyze,
  render,
  generate,
  disposeAnalysis,
};
