export function sliderValue(value: number, min: number, max: number, step: number): number {
  if (max <= min) return min;
  if (value >= max) return max;
  const stride = step > 0 && Number.isFinite(step) ? step : 1;
  const snapped = min + Math.round((value - min) / stride) * stride;
  return Math.min(max, Math.max(min, Number(snapped.toFixed(10))));
}
