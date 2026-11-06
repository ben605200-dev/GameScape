import { useEffect, useRef } from "react";

/**
 * Cursor-reactive wallpaper effect — the in-app rendering of the
 * "interactive wallpaper" prototype.
 *
 * On the desktop, the same effect is driven by the Rust cursor bridge
 * (see the Phase 0 spike harness); in this canvas it listens to pointer
 * movement directly, so it works wherever the app renders.
 */
export function InteractivePreview({ className }: { className?: string }) {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    let raf = 0;
    let W = 0;
    let H = 0;

    // Normalised target (0..1); rendered position lerps toward it.
    const target = { x: 0.5, y: 0.5 };
    const cur = { x: 0.5, y: 0.5 };
    const trail: { x: number; y: number; t: number }[] = [];

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

    const blobs = [
      { hue: 265, radius: 0.55, speed: 0.045 },
      { hue: 190, radius: 0.42, speed: 0.075 },
    ];

    const frame = (now: number) => {
      cur.x += (target.x - cur.x) * 0.08;
      cur.y += (target.y - cur.y) * 0.08;
      const cx = cur.x * W;
      const cy = cur.y * H;

      trail.push({ x: cx, y: cy, t: now });
      while (trail.length && now - trail[0].t > 650) trail.shift();

      const bg = ctx.createLinearGradient(0, 0, W, H);
      bg.addColorStop(0, "#07090d");
      bg.addColorStop(1, "#0c1016");
      ctx.fillStyle = bg;
      ctx.fillRect(0, 0, W, H);

      for (const blob of blobs) {
        const px = cx + Math.sin(now * blob.speed * 0.001) * W * 0.06;
        const py = cy + Math.cos(now * blob.speed * 0.0013) * H * 0.06;
        const radius = Math.max(W, H) * blob.radius;
        const g = ctx.createRadialGradient(px, py, 0, px, py, radius);
        g.addColorStop(0, `hsla(${blob.hue}, 90%, 60%, 0.34)`);
        g.addColorStop(1, "hsla(0, 0%, 0%, 0)");
        ctx.fillStyle = g;
        ctx.fillRect(0, 0, W, H);
      }

      for (let i = 0; i < trail.length; i += 1) {
        const point = trail[i];
        const ratio = i / trail.length;
        const alpha = ratio * ratio * 0.5;
        const r = 3 + 22 * ratio;
        const g = ctx.createRadialGradient(point.x, point.y, 0, point.x, point.y, r);
        g.addColorStop(0, `rgba(34, 211, 238, ${alpha})`);
        g.addColorStop(1, "rgba(34, 211, 238, 0)");
        ctx.fillStyle = g;
        ctx.beginPath();
        ctx.arc(point.x, point.y, r, 0, Math.PI * 2);
        ctx.fill();
      }

      const core = ctx.createRadialGradient(cx, cy, 0, cx, cy, 44);
      core.addColorStop(0, "rgba(240, 250, 255, 0.85)");
      core.addColorStop(0.25, "rgba(34, 211, 238, 0.55)");
      core.addColorStop(1, "rgba(124, 92, 255, 0)");
      ctx.fillStyle = core;
      ctx.beginPath();
      ctx.arc(cx, cy, 44, 0, Math.PI * 2);
      ctx.fill();

      raf = requestAnimationFrame(frame);
    };
    raf = requestAnimationFrame(frame);

    return () => {
      cancelAnimationFrame(raf);
      observer.disconnect();
      canvas.removeEventListener("pointermove", onMove);
    };
  }, []);

  return <canvas ref={canvasRef} className={className} />;
}
