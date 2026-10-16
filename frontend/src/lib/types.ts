export type Category =
  | 'image'
  | 'document'
  | 'spreadsheet'
  | 'presentation'
  | 'audio'
  | 'video'
  | 'archive';

export interface FormatInfo {
  format: string;
  label: string;
  extension: string;
  category: Category | (string & {});
  filename: string;
}

export interface TargetFormatInfo {
  format: string;
  label: string;
  extension: string;
  category: Category | (string & {});
  available: boolean;
  reason?: string | null;
}

export interface BatchInfo {
  files: FormatInfo[];
  common_targets: TargetFormatInfo[];
}

export interface ProgressPayload {
  job_id: string;
  progress: number;
  status: 'idle' | 'converting' | 'done' | 'cancelled' | 'error';
}

export interface ConvertResult {
  cancelled: boolean;
  success: boolean;
  output_path: string;
  target_format: string;
  error?: string | null;
}

export interface Options {
  quality: number;
  dpi: number;
  max_width?: number | null;
  max_height?: number | null;
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
