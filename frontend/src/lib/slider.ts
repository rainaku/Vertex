export function sliderValue(value: number, min: number, max: number, step: number): number {
  if (!Number.isFinite(value) || value <= min) return min;
  if (max <= min) return min;
  if (value >= max) return max;
  const stride = step > 0 && Number.isFinite(step) ? step : 1;
  const snapped = min + Math.round((value - min) / stride) * stride;
  const rounded = Number.isInteger(min) && Number.isInteger(stride) ? snapped : Math.round(snapped * 1e10) / 1e10;
  return Math.min(max, Math.max(min, rounded));
}
