import { useEffect, useRef } from "react";

/**
 * Cursor-reactive wallpaper previews.
 *
 * Three effect styles, tinted with each wallpaper's own placeholder colours:
 *  - orbit:  twin colour fields circling the cursor with a short trail
 *  - ribbon: a luminous light-trail flowing behind the cursor
 *  - grid:   an arcade grid that brightens around the cursor
 *
 * The same descriptors (`interactive`, `effect.style`) ride along in the
 * catalogue and will drive the desktop host in Phase 5; the rendering here
 * reacts to the in-window pointer directly.
 */

export type InteractiveEffect = "orbit" | "ribbon" | "grid";

interface InteractivePreviewProps {
  className?: string;
  effectName?: string;
  dominant?: string;
  accent?: string;
}

interface Rgb {
  r: number;
  g: number;
  b: number;
}

interface TrailPoint {
  x: number;
  y: number;
  t: number;
}

function parseHex(hex: string | undefined, fallback: Rgb): Rgb {
  if (!hex) return fallback;
  const match = /^#?([0-9a-f]{6})$/i.exec(hex.trim());
  if (!match) return fallback;
  const value = Number.parseInt(match[1], 16);
  return {
    r: (value >> 16) & 255,
    g: (value >> 8) & 255,
    b: value & 255,
  };
}

function rgba(color: Rgb, alpha: number): string {
  return `rgba(${color.r}, ${color.g}, ${color.b}, ${alpha})`;
}

function mix(a: Rgb, b: Rgb, t: number): Rgb {
  return {
    r: Math.round(a.r + (b.r - a.r) * t),
    g: Math.round(a.g + (b.g - a.g) * t),
    b: Math.round(a.b + (b.b - a.b) * t),
  };
}

function drawOrbit(
  ctx: CanvasRenderingContext2D,
  now: number,
  W: number,
  H: number,
  cx: number,
  cy: number,
  dominant: Rgb,
  accent: Rgb,
  trail: TrailPoint[],
): void {
  const base = Math.min(W, H);
  const blobs = [
    { color: dominant, radius: base * 0.62, phase: 0, speed: 0.0009, orbit: base * 0.3 },
    { color: accent, radius: base * 0.46, phase: Math.PI, speed: 0.0013, orbit: base * 0.2 },
  ];

  for (const blob of blobs) {
    // Clearly orbiting the cursor — alive even when the pointer rests.
    const px = cx + Math.cos(now * blob.speed + blob.phase) * blob.orbit;
    const py = cy + Math.sin(now * blob.speed * 1.4 + blob.phase) * blob.orbit * 0.7;
    const gradient = ctx.createRadialGradient(px, py, 0, px, py, blob.radius);
    gradient.addColorStop(0, rgba(blob.color, 0.4));
    gradient.addColorStop(1, rgba(blob.color, 0));
    ctx.fillStyle = gradient;
    ctx.fillRect(0, 0, W, H);
  }

  for (let i = 0; i < trail.length; i += 1) {
    const point = trail[i];
    const ratio = i / trail.length;
    const alpha = ratio * ratio * 0.5;
    const r = 3 + 22 * ratio;
    const gradient = ctx.createRadialGradient(point.x, point.y, 0, point.x, point.y, r);
    gradient.addColorStop(0, rgba(accent, alpha));
    gradient.addColorStop(1, rgba(accent, 0));
    ctx.fillStyle = gradient;
    ctx.beginPath();
    ctx.arc(point.x, point.y, r, 0, Math.PI * 2);
    ctx.fill();
  }

  const core = ctx.createRadialGradient(cx, cy, 0, cx, cy, 44);
  core.addColorStop(0, "rgba(240, 250, 255, 0.85)");
  core.addColorStop(0.25, rgba(accent, 0.55));
  core.addColorStop(1, rgba(dominant, 0));
  ctx.fillStyle = core;
  ctx.beginPath();
  ctx.arc(cx, cy, 44, 0, Math.PI * 2);
  ctx.fill();
}

function drawRibbon(
  ctx: CanvasRenderingContext2D,
  now: number,
  W: number,
  H: number,
  cx: number,
  cy: number,
  dominant: Rgb,
  accent: Rgb,
  trail: TrailPoint[],
): void {
  const halo = ctx.createRadialGradient(cx, cy, 0, cx, cy, Math.max(W, H) * 0.4);
  halo.addColorStop(0, rgba(dominant, 0.22));
  halo.addColorStop(1, rgba(dominant, 0));
  ctx.fillStyle = halo;
  ctx.fillRect(0, 0, W, H);

  const passes = [22, 15, 9, 4];
  const strokePath = (points: { x: number; y: number }[], intensity: number) => {
    for (let pass = 0; pass < passes.length; pass += 1) {
      ctx.lineCap = "round";
      for (let i = 1; i < points.length; i += 1) {
        const t = i / (points.length - 1);
        const point = points[i];
        const previous = points[i - 1];
        const color = mix(accent, dominant, t);
        ctx.strokeStyle = rgba(color, (0.05 + pass * 0.05) * t * intensity);
        ctx.lineWidth = passes[pass] * (0.4 + t * 0.6);
        ctx.beginPath();
        ctx.moveTo(previous.x, previous.y);
        ctx.lineTo(point.x, point.y);
        ctx.stroke();
      }
    }
  };

  // Idle: a flowing wave through the cursor so the ribbon is alive — and
  // clearly its own style — even when the pointer rests.
  const idleIntensity = Math.max(0.15, 1 - trail.length / 50) * 0.6;
  const idlePoints: { x: number; y: number }[] = [];
  const span = W * 0.4;
  for (let i = 0; i <= 28; i += 1) {
    const t = i / 28;
    const x = cx - span + t * span * 2;
    const y = cy + Math.sin(t * 5 + now * 0.0016) * H * 0.07;
    idlePoints.push({ x, y });
  }
  strokePath(idlePoints, idleIntensity);

  if (trail.length >= 2) {
    strokePath(trail, 1);
  }

  const head = ctx.createRadialGradient(cx, cy, 0, cx, cy, 20);
  head.addColorStop(0, "rgba(240, 250, 255, 0.9)");
  head.addColorStop(0.4, rgba(accent, 0.55));
  head.addColorStop(1, rgba(accent, 0));
  ctx.fillStyle = head;
  ctx.beginPath();
  ctx.arc(cx, cy, 20, 0, Math.PI * 2);
  ctx.fill();
}

function drawGrid(
  ctx: CanvasRenderingContext2D,
  now: number,
  W: number,
  H: number,
  cx: number,
  cy: number,
  dominant: Rgb,
  accent: Rgb,
): void {
  const halo = ctx.createRadialGradient(cx, cy, 0, cx, cy, Math.max(W, H) * 0.45);
  halo.addColorStop(0, rgba(dominant, 0.2));
  halo.addColorStop(1, rgba(dominant, 0));
  ctx.fillStyle = halo;
  ctx.fillRect(0, 0, W, H);

  const influence = Math.min(W, H) * 0.3;
  const pulse = 0.75 + Math.sin(now * 0.002) * 0.25;

  ctx.lineWidth = 1;
  const columns = 16;
  for (let i = 0; i <= columns; i += 1) {
    const x = (W / columns) * i;
    const distance = Math.abs(x - cx);
    const boost = Math.max(0, 1 - distance / influence);
    ctx.strokeStyle = rgba(accent, (0.07 + boost * 0.55) * pulse);
    ctx.beginPath();
    ctx.moveTo(x, 0);
    ctx.lineTo(x, H);
    ctx.stroke();
  }

  const rows = 10;
  for (let j = 0; j <= rows; j += 1) {
    const y = (H / rows) * j;
    const distance = Math.abs(y - cy);
    const boost = Math.max(0, 1 - distance / influence);
    ctx.strokeStyle = rgba(accent, (0.07 + boost * 0.55) * pulse);
    ctx.beginPath();
    ctx.moveTo(0, y);
    ctx.lineTo(W, y);
    ctx.stroke();
  }

  const node = ctx.createRadialGradient(cx, cy, 0, cx, cy, 16);
  node.addColorStop(0, "rgba(240, 250, 255, 0.9)");
  node.addColorStop(0.5, rgba(accent, 0.6));
  node.addColorStop(1, rgba(accent, 0));
  ctx.fillStyle = node;
  ctx.beginPath();
  ctx.arc(cx, cy, 16, 0, Math.PI * 2);
  ctx.fill();
}

export function InteractivePreview({
  className,
  effectName = "orbit",
  dominant,
  accent,
}: InteractivePreviewProps) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const dominantRgb = parseHex(dominant, { r: 124, g: 92, b: 255 });
    const accentRgb = parseHex(accent, { r: 34, g: 211, b: 238 });
    const effect: InteractiveEffect =
      effectName === "ribbon" || effectName === "grid" ? effectName : "orbit";

    let raf = 0;
    let W = 0;
    let H = 0;

    const target = { x: 0.5, y: 0.5 };
    // Start slightly off-centre so the effect glides in on first render.
    const cur = { x: 0.18, y: 0.72 };
    const trail: TrailPoint[] = [];

    const resize = () => {
      const rect = canvas.getBoundingClientRect();
      const dpr = window.devicePixelRatio || 1;
      W = rect.width;
      H = rect.height;
      canvas.width = Math.round(W * dpr);
      canvas.height = Math.round(H * dpr);
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    };
    resize();
    const observer = new ResizeObserver(resize);
    observer.observe(canvas);

    const onMove = (event: PointerEvent) => {
      const rect = canvas.getBoundingClientRect();
      target.x = (event.clientX - rect.left) / rect.width;
      target.y = (event.clientY - rect.top) / rect.height;
    };
    canvas.addEventListener("pointermove", onMove);

    const frame = (now: number) => {
      cur.x += (target.x - cur.x) * 0.08;
      cur.y += (target.y - cur.y) * 0.08;
      const cx = cur.x * W;
      const cy = cur.y * H;

      const lifetime = effect === "ribbon" ? 900 : 650;
      trail.push({ x: cx, y: cy, t: now });
      while (trail.length && now - trail[0].t > lifetime) trail.shift();

      const background = ctx.createLinearGradient(0, 0, W, H);
      background.addColorStop(0, "#07090d");
      background.addColorStop(1, "#0c1016");
      ctx.fillStyle = background;
      ctx.fillRect(0, 0, W, H);

      if (effect === "orbit") {
        drawOrbit(ctx, now, W, H, cx, cy, dominantRgb, accentRgb, trail);
      } else if (effect === "ribbon") {
        drawRibbon(ctx, now, W, H, cx, cy, dominantRgb, accentRgb, trail);
      } else {
        drawGrid(ctx, now, W, H, cx, cy, dominantRgb, accentRgb);
      }

      raf = requestAnimationFrame(frame);
    };
    raf = requestAnimationFrame(frame);

    return () => {
      cancelAnimationFrame(raf);
      observer.disconnect();
      canvas.removeEventListener("pointermove", onMove);
    };
  }, [effectName, dominant, accent]);

  return <canvas ref={canvasRef} className={className} />;
}
