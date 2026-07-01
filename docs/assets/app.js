const statusEl = document.getElementById("status");
const outputEl = document.getElementById("output");
const saveSvgBtn = document.getElementById("saveSvgButton");
const savePngBtn = document.getElementById("savePngButton");

const playBtn = document.getElementById("playBtn");
const playbackScrubber = document.getElementById("playbackScrubber");
const scrubberOverlay = document.getElementById("scrubberOverlay");
const configJsonCode = document.getElementById("configJsonCode");
const copyConfigBtn = document.getElementById("copyConfigBtn");

// Presets
const presetTechno = document.getElementById("presetTechno");
const presetAmbient = document.getElementById("presetAmbient");
const presetSweep = document.getElementById("presetSweep");

// File
const audioFileInput = document.getElementById("audioFile");
const dropZone = document.getElementById("dropZone");
const selectedFileEl = document.getElementById("selectedFile");

// Control Options
const shapeSelect = document.getElementById("shape");
const themeSelect = document.getElementById("theme");
const customColorsWrapper = document.getElementById("customColorsWrapper");
const customColorLow = document.getElementById("customColorLow");
const customColorMid = document.getElementById("customColorMid");
const customColorHigh = document.getElementById("customColorHigh");
const playbackRateInput = document.getElementById("playbackRate");
const playbackRateValue = document.getElementById("playbackRateValue");

// DSP options
const fftSizeSelect = document.getElementById("fftSize");
const normalizeModeSelect = document.getElementById("normalizeMode");
const detectionModeSelect = document.getElementById("detectionMode");
const framesPerColorInput = document.getElementById("framesPerColor");
const lowCutInput = document.getElementById("lowCut");
const midCutInput = document.getElementById("midCut");

// Metrics
const metricDecode = document.getElementById("metricDecode");
const metricAnalyze = document.getElementById("metricAnalyze");
const metricRender = document.getElementById("metricRender");
const metricFrames = document.getElementById("metricFrames");

// State variables
let initialized = false;
let wasmInit, wasmAnalyze, wasmAnalyzeWithOptions, wasmSvg, wasmPng;

let cachedDecode = null; // { pcm, sampleRate, duration, decodeTimeMs }
let latestAnalysis = null;
let activePresetName = "Techno";

let isPlaying = false;
let audioCtx = null;
let audioSourceNode = null;
let playbackStartTime = 0;
let playbackOffsetTime = 0;
let animationFrameId = null;
let audioWorker = null;
const presetCache = {};

const TARGET_SAMPLE_RATE = 11025;
const PRESET_DURATION = 90;

// Status helper
function setStatus(message, isError = false) {
  statusEl.textContent = message;
  statusEl.className = isError ? "status-bar error" : "status-bar";
}

// Initialize WASM
async function ensureWasmInit() {
  if (initialized) return;
  setStatus("Loading WASM DSP core...");
  const candidates = [
    "./moodbar-wasm/moodbar_wasm.js",
    "../../crates/moodbar-wasm/pkg/moodbar_wasm.js",
    "https://cdn.jsdelivr.net/npm/@moodbar/wasm@latest/moodbar_wasm.js",
  ];

  let lastError;
  for (const url of candidates) {
    try {
      const mod = await import(url);
      wasmInit = mod.default;
      wasmAnalyze = mod.analyze;
      wasmAnalyzeWithOptions = mod.analyze_with_options;
      wasmSvg = mod.svg;
      wasmPng = mod.png;
      break;
    } catch (error) {
      lastError = error;
    }
  }

  if (
    !wasmInit ||
    !wasmAnalyze ||
    !wasmSvg ||
    !wasmPng ||
    !wasmAnalyzeWithOptions
  ) {
    throw new Error("Failed to load WASM module. Check build or CDNs.");
  }

  await wasmInit();
  initialized = true;
}

// Get configuration objects
function getDspOptions() {
  const theme = themeSelect.value;
  const dsp = {
    fft_size: parseInt(fftSizeSelect.value),
    normalize_mode: normalizeModeSelect.value,
    detection_mode: detectionModeSelect.value,
    frames_per_color: parseInt(framesPerColorInput.value),
    playback_rate: parseFloat(playbackRateInput.value),
    band_edges_hz: [
      parseFloat(lowCutInput.value),
      parseFloat(midCutInput.value),
    ],
    low_cut_hz: parseFloat(lowCutInput.value),
    mid_cut_hz: parseFloat(midCutInput.value),
  };
  if (theme === "Custom") {
    dsp.custom_colors = [
      customColorLow.value,
      customColorMid.value,
      customColorHigh.value,
    ];
  } else {
    dsp.theme = theme;
  }
  return dsp;
}

function getRenderOptions() {
  const theme = themeSelect.value;
  const opts = {
    shape: shapeSelect.value,
    theme: theme,
  };
  if (theme === "Custom") {
    opts.custom_colors = [
      customColorLow.value,
      customColorMid.value,
      customColorHigh.value,
    ];
  }
  return opts;
}

function updateConfigJson() {
  const combined = {
    generator_options: getDspOptions(),
    renderer_options: getRenderOptions(),
  };
  configJsonCode.textContent = JSON.stringify(combined, null, 2);
}

// Audio presets generators (offloaded to Web Worker)
function generateSyntheticAudioInBackground(type, duration, sampleRate) {
  return new Promise((resolve) => {
    if (audioWorker) {
      audioWorker.terminate();
    }

    audioWorker = new Worker("./assets/computer-generated-sounds.js");

    audioWorker.onmessage = (e) => {
      resolve(e.data);
    };

    audioWorker.postMessage({ type, duration, sampleRate });
  });
}

async function loadPreset(name) {
  stopAudio();

  let pcm = presetCache[name];
  let elapsed = 0;

  if (!pcm) {
    setStatus(`Generating ${name} Loop...`);
    outputEl.innerHTML = `<div class="render-loading">Generating ${name} preset loop...</div>`;
    // Yield control to let UI paint status message first
    await new Promise((resolve) => setTimeout(resolve, 50));

    const t0 = performance.now();
    pcm = await generateSyntheticAudioInBackground(
      name,
      PRESET_DURATION,
      TARGET_SAMPLE_RATE,
    );
    elapsed = performance.now() - t0;
    presetCache[name] = pcm;
  } else {
    setStatus(`Loading ${name} Loop from cache...`);
    // Yield control to let UI paint status message first
    await new Promise((resolve) => setTimeout(resolve, 50));
  }

  cachedDecode = {
    pcm: pcm,
    sampleRate: TARGET_SAMPLE_RATE,
    duration: PRESET_DURATION,
    decodeTimeMs: elapsed,
  };

  activePresetName = name;
  selectedFileEl.textContent = `Preset: ${name} Loop`;
  audioFileInput.value = "";

  await runAnalysisAndRender();
}

// Web Audio API decoding
async function decodeFile(file) {
  stopAudio();
  setStatus("Reading file...");
  outputEl.innerHTML = '<div class="render-loading">Reading file...</div>';
  const arrayBuffer = await file.arrayBuffer();

  setStatus("Decoding audio channel...");
  outputEl.innerHTML =
    '<div class="render-loading">Decoding audio channel...</div>';

  // Yield control to let status text draw
  await new Promise((resolve) => setTimeout(resolve, 50));

  const t0 = performance.now();

  // Downsample directly to our target sample rate
  const decodeCtx = new OfflineAudioContext(1, 1, TARGET_SAMPLE_RATE);
  const audioBuffer = await decodeCtx.decodeAudioData(arrayBuffer);

  // Correctly calculate target length at the target sample rate instead of using audioBuffer.length
  const targetLength = Math.ceil(audioBuffer.duration * TARGET_SAMPLE_RATE);
  const monoCtx = new OfflineAudioContext(1, targetLength, TARGET_SAMPLE_RATE);
  const source = monoCtx.createBufferSource();
  source.buffer = audioBuffer;
  source.connect(monoCtx.destination);
  source.start(0);

  const renderedBuffer = await monoCtx.startRendering();
  const pcm = renderedBuffer.getChannelData(0).slice();
  const elapsed = performance.now() - t0;

  cachedDecode = {
    pcm: pcm,
    sampleRate: TARGET_SAMPLE_RATE,
    duration: audioBuffer.duration,
    decodeTimeMs: elapsed,
  };

  activePresetName = null;
  selectedFileEl.textContent = `File: ${file.name}`;

  await runAnalysisAndRender();
}

// Audio Playback Engine
function initAudioContext() {
  if (!audioCtx) {
    audioCtx = new (window.AudioContext || window.webkitAudioContext)();
  }
  if (audioCtx.state === "suspended") {
    void audioCtx.resume();
  }
}

function startAudio() {
  if (!cachedDecode) return;
  initAudioContext();

  // Stop current
  if (audioSourceNode) {
    try {
      audioSourceNode.stop();
    } catch (e) {}
  }

  // Create buffer
  const buffer = audioCtx.createBuffer(
    1,
    cachedDecode.pcm.length,
    cachedDecode.sampleRate,
  );
  buffer.getChannelData(0).set(cachedDecode.pcm);

  audioSourceNode = audioCtx.createBufferSource();
  audioSourceNode.buffer = buffer;
  audioSourceNode.connect(audioCtx.destination);

  // Set pitch
  const pRate = parseFloat(playbackRateInput.value);
  audioSourceNode.playbackRate.value = pRate;

  // Start from current offset
  if (playbackOffsetTime >= cachedDecode.duration || playbackOffsetTime < 0) {
    playbackOffsetTime = 0;
  }

  audioSourceNode.start(0, playbackOffsetTime);
  playbackStartTime = audioCtx.currentTime - playbackOffsetTime / pRate;

  isPlaying = true;
  playBtn.textContent = "Stop Audio";
  playbackScrubber.style.display = "block";

  audioSourceNode.onended = () => {
    // If ended naturally
    if (
      isPlaying &&
      (audioCtx.currentTime - playbackStartTime) * pRate >=
        cachedDecode.duration - 0.1
    ) {
      stopAudio();
    }
  };

  // Scrubber loop
  if (animationFrameId) cancelAnimationFrame(animationFrameId);
  tickScrubber();
}

function stopAudio() {
  isPlaying = false;
  playBtn.textContent = "Play Audio";
  playbackScrubber.style.display = "none";

  if (audioSourceNode) {
    try {
      audioSourceNode.stop();
    } catch (e) {}
    audioSourceNode = null;
  }
  if (animationFrameId) {
    cancelAnimationFrame(animationFrameId);
    animationFrameId = null;
  }
}

function seekAudio(percent) {
  if (!cachedDecode) return;
  playbackOffsetTime = percent * cachedDecode.duration;

  if (isPlaying) {
    startAudio();
  } else {
    // Just update visual scrubber position
    updateScrubberVisual(percent);
    playbackScrubber.style.display = "block";
  }
}

function updateScrubberVisual(percent) {
  playbackScrubber.style.left = `${percent * 100}%`;
}

function tickScrubber() {
  if (!isPlaying || !cachedDecode) return;
  const pRate = parseFloat(playbackRateInput.value);
  const elapsed = (audioCtx.currentTime - playbackStartTime) * pRate;
  const percent = Math.min(elapsed / cachedDecode.duration, 1);

  updateScrubberVisual(percent);

  if (percent < 1) {
    animationFrameId = requestAnimationFrame(tickScrubber);
  } else {
    stopAudio();
  }
}

// Analysis & Rendering
async function runAnalysisAndRender() {
  if (!cachedDecode) return;

  try {
    await ensureWasmInit();
    setStatus("Running audio analysis...");
    outputEl.innerHTML =
      '<div class="render-loading">Running audio analysis...</div>';

    const dspOpts = getDspOptions();

    metricDecode.textContent = `${(cachedDecode.decodeTimeMs / 1000).toFixed(2)}s`;

    // 1. Analyze PCM (high-res, max 1000 frames)
    const t0 = performance.now();
    if (latestAnalysis) {
      latestAnalysis.free();
      latestAnalysis = null;
    }
    const mainDspOpts = { ...dspOpts, max_target_frames: 1000 };
    latestAnalysis = wasmAnalyzeWithOptions(
      cachedDecode.pcm,
      cachedDecode.sampleRate,
      mainDspOpts,
    );
    const tAnalyze = performance.now() - t0;
    metricAnalyze.textContent = `${(tAnalyze / 1000).toFixed(3)}s`;
    metricFrames.textContent = `${latestAnalysis.frame_count()} frames`;

    // 2. Render Main SVG (synchronous, instant feedback)
    setStatus("Rendering active moodbar...");
    outputEl.innerHTML =
      '<div class="render-loading">Rendering active moodbar...</div>';
    const t1 = performance.now();

    const svgMarkup = wasmSvg(latestAnalysis, {
      width: 900,
      height: 120,
      shape: shapeSelect.value,
      background: "transparent",
    });
    outputEl.innerHTML = svgMarkup;
    const tRender = performance.now() - t1;
    metricRender.textContent = `${(tRender / 1000).toFixed(3)}s`;

    savePngBtn.disabled = false;
    updateConfigJson();
    setStatus("Ready.");
  } catch (error) {
    console.error(error);
    setStatus(error.message, true);
  }
}

// Download utils
function downloadSvg() {
  if (!latestAnalysis) return;
  const svgMarkup = wasmSvg(latestAnalysis, {
    width: 1200,
    height: 160,
    shape: shapeSelect.value,
    background: "transparent",
  });
  const blob = new Blob([svgMarkup], { type: "image/svg+xml" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = `${activePresetName || "moodbar"}_signature.svg`;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

function downloadPng() {
  if (!latestAnalysis) return;
  try {
    const pngBytes = wasmPng(latestAnalysis, {
      width: 1200,
      height: 160,
      shape: shapeSelect.value,
    });
    const blob = new Blob([pngBytes], { type: "image/png" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `${activePresetName || "moodbar"}_signature.png`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);
  } catch (e) {
    setStatus(`PNG export failed: ${e.message}`, true);
  }
}

// Event Listeners: Presets
const presetButtons = {
  Techno: presetTechno,
  Ambient: presetAmbient,
  Sweep: presetSweep,
};
Object.entries(presetButtons).forEach(([name, btn]) => {
  btn.addEventListener("click", () => {
    Object.values(presetButtons).forEach((b) => b.classList.remove("active"));
    btn.classList.add("active");
    void loadPreset(name);
  });
});

// Helper for file handling
function handleAudioFileSelected(file) {
  if (!file) return;
  Object.values(presetButtons).forEach((b) => b.classList.remove("active"));
  setStatus("Reading file...");
  setTimeout(() => {
    void decodeFile(file);
  }, 50);
}

// Event Listeners: File drop
dropZone.addEventListener("click", () => audioFileInput.click());
audioFileInput.addEventListener("change", () => {
  handleAudioFileSelected(audioFileInput.files?.[0]);
});

dropZone.addEventListener("dragover", (e) => {
  e.preventDefault();
  dropZone.classList.add("active");
});
dropZone.addEventListener("dragleave", () => {
  dropZone.classList.remove("active");
});
dropZone.addEventListener("drop", (e) => {
  e.preventDefault();
  dropZone.classList.remove("active");
  handleAudioFileSelected(e.dataTransfer?.files?.[0]);
});

// Event Listeners: Visual settings
shapeSelect.addEventListener("change", () => {
  void runAnalysisAndRender();
});
themeSelect.addEventListener("change", () => {
  if (themeSelect.value === "Custom") {
    customColorsWrapper.style.display = "block";
  } else {
    customColorsWrapper.style.display = "none";
  }
  void runAnalysisAndRender();
});

[customColorLow, customColorMid, customColorHigh].forEach((picker) => {
  picker.addEventListener("input", () => {
    void runAnalysisAndRender();
  });
});

playbackRateInput.addEventListener("input", () => {
  const val = parseFloat(playbackRateInput.value);
  playbackRateValue.textContent = `${val.toFixed(2)}x`;
  if (audioSourceNode && isPlaying) {
    audioSourceNode.playbackRate.value = val;
  }
  void runAnalysisAndRender();
});

// Event Listeners: DSP parameters
[
  fftSizeSelect,
  normalizeModeSelect,
  detectionModeSelect,
  framesPerColorInput,
  lowCutInput,
  midCutInput,
].forEach((ctrl) => {
  ctrl.addEventListener("change", () => {
    void runAnalysisAndRender();
  });
});

// Playback click / Scrubber seek
scrubberOverlay.addEventListener("click", (e) => {
  const rect = scrubberOverlay.getBoundingClientRect();
  const x = e.clientX - rect.left;
  const percent = Math.max(0, Math.min(x / rect.width, 1));
  seekAudio(percent);
});

// Actions
playBtn.addEventListener("click", () => {
  if (isPlaying) {
    stopAudio();
  } else {
    startAudio();
  }
});
saveSvgBtn.addEventListener("click", downloadSvg);
savePngBtn.addEventListener("click", downloadPng);

// Copy config
copyConfigBtn.addEventListener("click", () => {
  navigator.clipboard.writeText(configJsonCode.textContent).then(() => {
    const oldText = copyConfigBtn.textContent;
    copyConfigBtn.textContent = "Copied!";
    setTimeout(() => (copyConfigBtn.textContent = oldText), 1500);
  });
});

// Document Tabs
const tabHeaders = document.querySelectorAll(".tab-header");
tabHeaders.forEach((header) => {
  header.addEventListener("click", () => {
    // Remove active
    tabHeaders.forEach((h) => h.classList.remove("active"));
    document
      .querySelectorAll(".tab-panel")
      .forEach((p) => p.classList.remove("active"));

    // Add active
    header.classList.add("active");
    const tabId = header.getAttribute("data-tab");
    document.getElementById(tabId).classList.add("active");
  });
});

// Hash-based Tab Routing
function handleHashChange() {
  const hash = window.location.hash.toLowerCase();
  let targetTabId = "";

  if (hash === "#cli") {
    targetTabId = "cli-tab";
  } else if (hash === "#wasm") {
    targetTabId = "wasm-tab";
  } else if (hash === "#mobile") {
    targetTabId = "mobile-tab";
  } else if (hash === "#ffi") {
    targetTabId = "ffi-tab";
  }

  if (targetTabId) {
    const header = Array.from(tabHeaders).find(
      (h) => h.getAttribute("data-tab") === targetTabId,
    );
    if (header) {
      header.click();
      // Scroll to integration section
      const integrationSection = document.getElementById("integration");
      if (integrationSection) {
        integrationSection.scrollIntoView({ behavior: "smooth" });
      }
    }
  }
}
window.addEventListener("hashchange", handleHashChange);
// Run on DOM load and full page load
window.addEventListener("load", handleHashChange);

// Lightbox functionality
const lightbox = document.getElementById("lightbox");
const lightboxImg = document.getElementById("lightboxImage");
const lightboxClose = document.getElementById("lightboxClose");

// Add pointer cursor to preview images in gallery to show they are clickable
const galleryPreviews = document.querySelectorAll(".showcase-preview img");
galleryPreviews.forEach((img) => {
  img.style.cursor = "zoom-in";
  img.addEventListener("click", () => {
    lightboxImg.src = img.src;
    lightboxImg.alt = img.alt;
    lightbox.classList.add("active");
  });
});

lightboxClose.addEventListener("click", () => {
  lightbox.classList.remove("active");
});

lightbox.addEventListener("click", (e) => {
  if (e.target === lightbox) {
    lightbox.classList.remove("active");
  }
});

// Escape key to close
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape" && lightbox.classList.contains("active")) {
    lightbox.classList.remove("active");
  }
});

// Startup
window.addEventListener("DOMContentLoaded", async () => {
  try {
    await ensureWasmInit();
    void loadPreset("Techno");
  } catch (e) {
    setStatus(e.message, true);
  }
});
