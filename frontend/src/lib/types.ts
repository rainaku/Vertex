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
}
