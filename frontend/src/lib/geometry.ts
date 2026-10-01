export interface Point {
  x: number;
  y: number;
}

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
  total: number = 8,
  cx: number = 260,
  cy: number = 260,
  rIn: number = 70,
  rOut: number = 240,
  cornerRadius: number = 12
): string {
  const sectorAngle = (2 * Math.PI) / total;
  const angularGap = 0.045; // ~2.5 degrees gap between petals

  const startAngle = index * sectorAngle - Math.PI / 2 + angularGap / 2;
  const endAngle = (index + 1) * sectorAngle - Math.PI / 2 - angularGap / 2;

  // Angular offsets for corner radius at inner and outer radii
  const dAngleIn = cornerRadius / rIn;
  const dAngleOut = cornerRadius / rOut;

  // Corner 1: Inner Start (junction of inner arc and start radial line)
  const pRadialStart = polarToCartesian(cx, cy, rIn + cornerRadius, startAngle);
  const pRadialEnd = polarToCartesian(cx, cy, rOut - cornerRadius, startAngle);

  // Corner 2: Outer Start (junction of start radial line and outer arc)
  const vertexOuterStart = polarToCartesian(cx, cy, rOut, startAngle);
  const pOuterArcStart = polarToCartesian(cx, cy, rOut, startAngle + dAngleOut);

  // Outer Arc segment
  const pOuterArcEnd = polarToCartesian(cx, cy, rOut, endAngle - dAngleOut);

  // Corner 3: Outer End (junction of outer arc and end radial line)
  const vertexOuterEnd = polarToCartesian(cx, cy, rOut, endAngle);
  const pRadialEnd2 = polarToCartesian(cx, cy, rOut - cornerRadius, endAngle);

  // Radial line 2 inward
  const pRadialStart2 = polarToCartesian(cx, cy, rIn + cornerRadius, endAngle);

  // Corner 4: Inner End (junction of end radial line and inner arc)
  const vertexInnerEnd = polarToCartesian(cx, cy, rIn, endAngle);
  const pInnerArcEnd = polarToCartesian(cx, cy, rIn, endAngle - dAngleIn);

  // Inner Arc segment back toward startAngle
  const pInnerArcStart = polarToCartesian(cx, cy, rIn, startAngle + dAngleIn);

  // Inner Start corner control vertex
  const vertexInnerStart = polarToCartesian(cx, cy, rIn, startAngle);

  return [
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
}

/**
 * Calculates the center point of a petal for label placement.
 */
export function getPetalCenter(
  index: number,
  total: number = 8,
  cx: number = 260,
  cy: number = 260,
  rMid: number = 155
): { x: number; y: number; angleDeg: number } {
  const sectorAngle = (2 * Math.PI) / total;
  const midAngle = index * sectorAngle - Math.PI / 2 + sectorAngle / 2;
  const pos = polarToCartesian(cx, cy, rMid, midAngle);
  const angleDeg = (midAngle * 180) / Math.PI;

  return {
    x: pos.x,
    y: pos.y,
    angleDeg,
  };
}

/**
 * Hit test a point (x, y) relative to the 520x520 SVG coordinate space.
 */
export function hitTest(
  x: number,
  y: number,
  cx: number = 260,
  cy: number = 260,
  rIn: number = 66,
  rOut: number = 246,
  total: number = 8
): { type: 'center' | 'petal' | 'outside'; index: number } {
  const dx = x - cx;
  const dy = y - cy;
  const dist = Math.sqrt(dx * dx + dy * dy);

  if (dist < rIn) {
    return { type: 'center', index: -1 };
  }

  if (dist > rOut) {
    return { type: 'outside', index: -1 };
  }

  // Calculate angle, offset so -PI/2 (top 12 o'clock) is 0
  let angle = Math.atan2(dy, dx) + Math.PI / 2;
  while (angle < 0) angle += 2 * Math.PI;
  while (angle >= 2 * Math.PI) angle -= 2 * Math.PI;

  const sectorAngle = (2 * Math.PI) / total;
  const index = Math.floor(angle / sectorAngle) % total;

  return { type: 'petal', index };
}
