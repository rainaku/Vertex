export interface FormatInfo {
  format: string;
  label: string;
  extension: string;
  category: string;
  filename: string;
}

export interface TargetFormatInfo {
  format: string;
  label: string;
  extension: string;
  category: string;
  available: boolean;
  reason?: string;
}

export interface BatchInfo {
  files: FormatInfo[];
  common_targets: TargetFormatInfo[];
}

export interface ProgressPayload {
  job_id: string;
  progress: number;
  status: 'idle' | 'converting' | 'done' | 'error';
}

export interface ConvertResult {
  cancelled?: boolean;
  success: boolean;
  output_path: string;
  target_format: string;
  error?: string;
}

export interface Options {
  quality: number;
  dpi: number;
  max_width?: number;
  max_height?: number;
  strip_metadata: boolean;
  output_dir?: string;
  collision_policy: 'rename_with_suffix' | 'fail_if_exists' | 'overwrite';
  jpeg_background?: [number, number, number];
  gif_alpha_threshold?: number;
  avif_speed?: number;
  video_codec?: 'h264' | 'h265';
  video_preset?: 'fast' | 'medium' | 'slow';
  video_crf?: number;
  video_audio_kbps?: number;
}
