export type NormalizeMode = "PerChannelPeak" | "GlobalPeak";
export type DetectionMode = "SpectralEnergy" | "SpectralFlux";
export type SvgShape = "Strip" | "Waveform";

export interface AnalyzeOptions {
  fft_size?: number;
  low_cut_hz?: number;
  mid_cut_hz?: number;
  normalize_mode?: NormalizeMode;
  deterministic_floor?: number;
  detection_mode?: DetectionMode;
  frames_per_color?: number;
  band_edges_hz?: number[];
}

export interface RenderOptions {
  width?: number;
  height?: number;
  shape?: SvgShape;
  background?: "transparent" | "black" | "white" | "none";
  max_gradient_stops?: number;
}

export type MoodbarInput =
  | { uri: string }
  | { bytes: Uint8Array; name?: string; mimeType?: string };

export interface MoodbarAnalysis {
  readonly frameCount: number;
  readonly channelCount: number;
  readonly disposed: boolean;
  dispose(): Promise<void>;
}

export function analyze(
  input: MoodbarInput,
  options?: AnalyzeOptions
): Promise<MoodbarAnalysis>;

export function render(
  analysis: MoodbarAnalysis,
  format: "png",
  options?: RenderOptions
): Promise<Uint8Array>;
export function render(
  analysis: MoodbarAnalysis,
  format: "svg",
  options?: RenderOptions
): Promise<string>;

export function generate(
  input: MoodbarInput,
  format: "png",
  options?: AnalyzeOptions & RenderOptions
): Promise<Uint8Array>;
export function generate(
  input: MoodbarInput,
  format: "svg",
  options?: AnalyzeOptions & RenderOptions
): Promise<string>;

export function disposeAnalysis(analysis: MoodbarAnalysis): Promise<void>;
