import { libraryReadData, librarySetPoster, type LibraryItem } from "./ipc";

/**
 * Poster generation: the first playable frame of a video becomes its
 * thumbnail. WebView2 decodes the video, a canvas captures the frame, and the
 * Rust core validates and stores the JPEG. Runs lazily — only for video items
 * that don't have a thumbnail yet, once per session per item.
 */

const attempted = new Set<number>();

export function generateMissingPosters(items: LibraryItem[]): void {
  for (const item of items) {
    if (item.format !== "mp4" || item.thumbPath || attempted.has(item.id)) {
      continue;
    }
    attempted.add(item.id);
    void generatePoster(item).catch((error) => {
      console.warn("poster generation failed", item.id, error);
    });
  }
}

async function generatePoster(item: LibraryItem): Promise<void> {
  // Load through a data URL: same-origin for the canvas, unlike the
  // cross-origin asset protocol (which would taint it and block capture).
  const data = await libraryReadData(item.id);
  if (!data.ok) {
    throw new Error(data.error);
  }
  const source = data.value;

  const video = document.createElement("video");
  video.muted = true;
  video.preload = "auto";
  video.src = source;

  await new Promise<void>((resolve, reject) => {
    const timer = window.setTimeout(
      () => reject(new Error("video load timed out")),
      10000,
    );
    video.onloadeddata = () => {
      window.clearTimeout(timer);
      resolve();
    };
    video.onerror = () => {
      window.clearTimeout(timer);
      reject(new Error("video failed to load"));
    };
  });

  // Seek slightly in so the frame is representative rather than black.
  const duration = Number.isFinite(video.duration) ? video.duration : 2;
  const seekTo = Math.min(1, Math.max(0.1, duration * 0.1));
  await new Promise<void>((resolve, reject) => {
    const timer = window.setTimeout(
      () => reject(new Error("seek timed out")),
      10000,
    );
    video.onseeked = () => {
      window.clearTimeout(timer);
      resolve();
    };
    video.currentTime = seekTo;
  });

  const width = 480;
  const height = Math.max(
    1,
    Math.round((video.videoHeight / Math.max(1, video.videoWidth)) * width),
  );
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const context = canvas.getContext("2d");
  if (!context) return;
  context.drawImage(video, 0, 0, width, height);

  const dataUrl = canvas.toDataURL("image/jpeg", 0.82);
  const result = await librarySetPoster(item.id, dataUrl);
  if (!result.ok) {
    throw new Error(result.error);
  }

  // Let any open library view pick up the new thumbnail immediately.
  window.dispatchEvent(new CustomEvent("gamescape:library-refresh"));
}
