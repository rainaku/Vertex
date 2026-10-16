export const WHEEL_VIEWBOX_SIZE = 520;
export const WHEEL_CX = 260;
export const WHEEL_CY = 260;
export const WHEEL_R_IN = 70;
export const WHEEL_R_OUT = 244;
export const WHEEL_TOTAL_PETALS = 8;

export interface Point {
  x: number;
  y: number;
}

export interface HitResult {
  readonly type: 'center' | 'petal' | 'outside';
  readonly index: number;
}

/** The DOM rectangle already includes CSS transforms and the current DPI. */
export function hitTestClient(clientX: number, clientY: number,
  rect: Pick<DOMRect, 'left' | 'top' | 'width' | 'height'>): HitResult {
  if (rect.width <= 0 || rect.height <= 0) return HIT_OUTSIDE;
  return hitTest((clientX - rect.left) * WHEEL_VIEWBOX_SIZE / rect.width,
    (clientY - rect.top) * WHEEL_VIEWBOX_SIZE / rect.height);
}

const HIT_CENTER: HitResult = Object.freeze({ type: 'center', index: -1 });
const HIT_OUTSIDE: HitResult = Object.freeze({ type: 'outside', index: -1 });
const HIT_PETAL_CACHE: readonly HitResult[] = Object.freeze(
  Array.from({ length: 32 }, (_, index) =>
    Object.freeze({ type: 'petal' as const, index })
  )
);

const petalPathCache = new Map<string, string>();
const petalCenterCache = new Map<
  string,
  { readonly x: number; readonly y: number; readonly angleDeg: number }
>();

export function polarToCartesian(
  cx: number,
  cy: number,
  r: number,
  angleRad: number
): Point {
  return {
    x: cx + r * Math.cos(angleRad),
    y: cy + r * Math.sin(angleRad),
  };
}

/**
 * Generates an SVG path data string for a sector petal with rounded corners.
 */
export function createPetalPath(
  index: number,
  total: number = WHEEL_TOTAL_PETALS,
  cx: number = WHEEL_CX,
  cy: number = WHEEL_CY,
  rIn: number = WHEEL_R_IN,
  rOut: number = WHEEL_R_OUT,
  cornerRadius: number = 12
): string {
  const cacheKey = `${index}:${total}:${cx}:${cy}:${rIn}:${rOut}:${cornerRadius}`;
  const cached = petalPathCache.get(cacheKey);
  if (cached !== undefined) {
    return cached;
  }

  const sectorAngle = (2 * Math.PI) / total;
  const angularGap = 0.045; // ~2.5 degrees gap between petals

  const startAngle = index * sectorAngle - Math.PI / 2 + angularGap / 2;
  const endAngle = (index + 1) * sectorAngle - Math.PI / 2 - angularGap / 2;

  const cosStart = Math.cos(startAngle);
  const sinStart = Math.sin(startAngle);
  const cosEnd = Math.cos(endAngle);
  const sinEnd = Math.sin(endAngle);

  // Angular offsets for corner radius at inner and outer radii
  const dAngleIn = cornerRadius / rIn;
  const dAngleOut = cornerRadius / rOut;

  const rInPlus = rIn + cornerRadius;
  const rOutMinus = rOut - cornerRadius;

  // Corner 1: Inner Start (junction of inner arc and start radial line)
  const pRadialStart = {
    x: cx + rInPlus * cosStart,
    y: cy + rInPlus * sinStart,
  };
  const pRadialEnd = {
    x: cx + rOutMinus * cosStart,
    y: cy + rOutMinus * sinStart,
  };

  // Corner 2: Outer Start (junction of start radial line and outer arc)
  const vertexOuterStart = {
    x: cx + rOut * cosStart,
    y: cy + rOut * sinStart,
  };
  const pOuterArcStart = polarToCartesian(cx, cy, rOut, startAngle + dAngleOut);

  // Outer Arc segment
  const pOuterArcEnd = polarToCartesian(cx, cy, rOut, endAngle - dAngleOut);

  // Corner 3: Outer End (junction of outer arc and end radial line)
  const vertexOuterEnd = {
    x: cx + rOut * cosEnd,
    y: cy + rOut * sinEnd,
  };
  const pRadialEnd2 = {
    x: cx + rOutMinus * cosEnd,
    y: cy + rOutMinus * sinEnd,
  };

  // Radial line 2 inward
  const pRadialStart2 = {
    x: cx + rInPlus * cosEnd,
    y: cy + rInPlus * sinEnd,
  };

  // Corner 4: Inner End (junction of end radial line and inner arc)
  const vertexInnerEnd = {
    x: cx + rIn * cosEnd,
    y: cy + rIn * sinEnd,
  };
  const pInnerArcEnd = polarToCartesian(cx, cy, rIn, endAngle - dAngleIn);

  // Inner Arc segment back toward startAngle
  const pInnerArcStart = polarToCartesian(cx, cy, rIn, startAngle + dAngleIn);

  // Inner Start corner control vertex
  const vertexInnerStart = {
    x: cx + rIn * cosStart,
    y: cy + rIn * sinStart,
  };

  const path = [
    `M ${pRadialStart.x.toFixed(2)} ${pRadialStart.y.toFixed(2)}`,
    // Straight radial line outward
    `L ${pRadialEnd.x.toFixed(2)} ${pRadialEnd.y.toFixed(2)}`,
    // Outer start rounded corner
    `Q ${vertexOuterStart.x.toFixed(2)} ${vertexOuterStart.y.toFixed(2)} ${pOuterArcStart.x.toFixed(2)} ${pOuterArcStart.y.toFixed(2)}`,
    // Circular outer arc along rOut
    `A ${rOut} ${rOut} 0 0 1 ${pOuterArcEnd.x.toFixed(2)} ${pOuterArcEnd.y.toFixed(2)}`,
    // Outer end rounded corner
    `Q ${vertexOuterEnd.x.toFixed(2)} ${vertexOuterEnd.y.toFixed(2)} ${pRadialEnd2.x.toFixed(2)} ${pRadialEnd2.y.toFixed(2)}`,
    // Straight radial line inward
    `L ${pRadialStart2.x.toFixed(2)} ${pRadialStart2.y.toFixed(2)}`,
    // Inner end rounded corner
    `Q ${vertexInnerEnd.x.toFixed(2)} ${vertexInnerEnd.y.toFixed(2)} ${pInnerArcEnd.x.toFixed(2)} ${pInnerArcEnd.y.toFixed(2)}`,
    // Circular inner arc along rIn
    `A ${rIn} ${rIn} 0 0 0 ${pInnerArcStart.x.toFixed(2)} ${pInnerArcStart.y.toFixed(2)}`,
    // Inner start rounded corner seamlessly back to pRadialStart
    `Q ${vertexInnerStart.x.toFixed(2)} ${vertexInnerStart.y.toFixed(2)} ${pRadialStart.x.toFixed(2)} ${pRadialStart.y.toFixed(2)}`,
    `Z`,
  ].join(' ');

  petalPathCache.set(cacheKey, path);
  return path;
}

/**
 * Calculates the center point of a petal for label placement.
 */
export function getPetalCenter(
  index: number,
  total: number = WHEEL_TOTAL_PETALS,
  cx: number = WHEEL_CX,
  cy: number = WHEEL_CY,
  rMid: number = 155
): { x: number; y: number; angleDeg: number } {
  const cacheKey = `${index}:${total}:${cx}:${cy}:${rMid}`;
  const cached = petalCenterCache.get(cacheKey);
  if (cached !== undefined) {
    return cached;
  }

  const sectorAngle = (2 * Math.PI) / total;
  const midAngle = index * sectorAngle - Math.PI / 2 + sectorAngle / 2;
  const pos = polarToCartesian(cx, cy, rMid, midAngle);
  const angleDeg = (midAngle * 180) / Math.PI;

  const result = Object.freeze({
    x: pos.x,
    y: pos.y,
    angleDeg,
  });
  petalCenterCache.set(cacheKey, result);
  return result;
}

/**
 * Hit test a point (x, y) relative to the 520x520 SVG coordinate space.
 */
export function hitTest(
  x: number,
  y: number,
  cx: number = WHEEL_CX,
  cy: number = WHEEL_CY,
  rIn: number = WHEEL_R_IN,
  rOut: number = WHEEL_R_OUT,
  total: number = WHEEL_TOTAL_PETALS
): HitResult {
  const dx = x - cx;
  const dy = y - cy;
  const distSq = dx * dx + dy * dy;

  if (distSq < rIn * rIn) {
    return HIT_CENTER;
  }

  if (distSq > rOut * rOut) {
    return HIT_OUTSIDE;
  }

  // Calculate angle, offset so -PI/2 (top 12 o'clock) is 0
  let angle = Math.atan2(dy, dx) + Math.PI / 2;
  if (angle < 0) angle += 2 * Math.PI;
  if (angle >= 2 * Math.PI) angle -= 2 * Math.PI;

  const sectorAngle = (2 * Math.PI) / total;
  const index = Math.floor(angle / sectorAngle) % total;

  return HIT_PETAL_CACHE[index] ?? { type: 'petal', index };
}
