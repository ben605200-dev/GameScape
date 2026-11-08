import {
  assetUrl,
  libraryReadData,
  librarySetPoster,
  type LibraryItem,
} from "./ipc";

/**
 * Thumbnail generation.
 *
 * Videos: the first playable frame becomes the thumbnail. Images without a
 * thumbnail (e.g. catalogue entries rendered into the library) are
 * downscaled instead. Both load through the asset protocol with
 * `crossOrigin = "anonymous"` — the protocol sends the CORS headers the
 * canvas needs to stay untainted, so capture works for any file size.
 * Runs lazily, once per session per item.
 */

const attempted = new Set<number>();

const IMAGE_FORMATS = new Set(["png", "jpg", "jpeg", "webp", "gif"]);

export function generateMissingPosters(items: LibraryItem[]): void {
  for (const item of items) {
    if (item.thumbPath || attempted.has(item.id) || !item.filePath) {
      continue;
    }
    const isVideo = item.format === "mp4";
    const isImage = item.format ? IMAGE_FORMATS.has(item.format) : false;
    if (!isVideo && !isImage) {
      continue;
    }
    attempted.add(item.id);
    const task = isVideo ? generateVideoPoster(item) : generateImageThumb(item);
    void task.catch((error) => {
      console.warn("thumbnail generation failed", item.id, error);
    });
  }
}

/** Draw the source into a 480-wide JPEG; null when capture is blocked. */
function captureFrame(
  source: HTMLImageElement | HTMLVideoElement,
  width: number,
  height: number,
): string | null {
  if (!width || !height) return null;
  const targetWidth = 480;
  const targetHeight = Math.max(
    1,
    Math.round((height / Math.max(1, width)) * targetWidth),
  );
  const canvas = document.createElement("canvas");
  canvas.width = targetWidth;
  canvas.height = targetHeight;
  const context = canvas.getContext("2d");
  if (!context) return null;
  context.drawImage(source, 0, 0, targetWidth, targetHeight);
  try {
    return canvas.toDataURL("image/jpeg", 0.82);
  } catch {
    return null;
  }
}

function loadImage(item: LibraryItem): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const image = new Image();
    image.crossOrigin = "anonymous";
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error("image failed to load"));
    image.src = assetUrl(item.filePath!);
  });
}

function loadVideo(item: LibraryItem): Promise<HTMLVideoElement> {
  return new Promise((resolve, reject) => {
    const video = document.createElement("video");
    video.crossOrigin = "anonymous";
    video.muted = true;
    video.preload = "auto";
    const timer = window.setTimeout(
      () => reject(new Error("video load timed out")),
      15000,
    );
    video.onloadeddata = () => {
      window.clearTimeout(timer);
      resolve(video);
    };
    video.onerror = () => {
      window.clearTimeout(timer);
      reject(new Error("video failed to load"));
    };
    video.src = assetUrl(item.filePath!);
  });
}

function seekVideo(video: HTMLVideoElement): Promise<void> {
  // Seek slightly in so the frame is representative rather than black.
  const duration = Number.isFinite(video.duration) ? video.duration : 2;
  const seekTo = Math.min(1, Math.max(0.1, duration * 0.1));
  return new Promise((resolve, reject) => {
    const timer = window.setTimeout(
      () => reject(new Error("seek timed out")),
      15000,
    );
    video.onseeked = () => {
      window.clearTimeout(timer);
      resolve();
    };
    video.currentTime = seekTo;
  });
}

async function storeThumb(item: LibraryItem, dataUrl: string | null) {
  if (!dataUrl) return;
  const result = await librarySetPoster(item.id, dataUrl);
  if (!result.ok) {
    throw new Error(result.error);
  }
  // Let any open library view pick up the new thumbnail immediately.
  window.dispatchEvent(new CustomEvent("gamescape:library-refresh"));
}

async function generateImageThumb(item: LibraryItem): Promise<void> {
  const image = await loadImage(item);
  await storeThumb(
    item,
    captureFrame(image, image.naturalWidth, image.naturalHeight),
  );
}

async function generateVideoPoster(item: LibraryItem): Promise<void> {
  let dataUrl: string | null = null;
  let video: HTMLVideoElement | null = null;
  try {
    video = await loadVideo(item);
    await seekVideo(video);
    dataUrl = captureFrame(video, video.videoWidth, video.videoHeight);
  } catch {
    dataUrl = null;
  } finally {
    if (video) {
      video.removeAttribute("src");
      video.load();
    }
  }

  if (!dataUrl) {
    // Fallback for anywhere CORS capture fails: the same-origin, size-capped
    // data pipeline (large files are refused by design).
    const data = await libraryReadData(item.id);
    if (!data.ok) return;
    const fallback = document.createElement("video");
    fallback.muted = true;
    fallback.preload = "auto";
    fallback.src = data.value;
    await new Promise<void>((resolve, reject) => {
      const timer = window.setTimeout(
        () => reject(new Error("video load timed out")),
        15000,
      );
      fallback.onloadeddata = () => {
        window.clearTimeout(timer);
        resolve();
      };
      fallback.onerror = () => {
        window.clearTimeout(timer);
        reject(new Error("video failed to load"));
      };
    });
    await seekVideo(fallback);
    dataUrl = captureFrame(fallback, fallback.videoWidth, fallback.videoHeight);
    fallback.removeAttribute("src");
    fallback.load();
  }

  await storeThumb(item, dataUrl);
}
