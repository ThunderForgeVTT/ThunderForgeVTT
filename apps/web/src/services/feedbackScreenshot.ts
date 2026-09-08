/**
 * The optional screenshot (spec 037, FR-008 and FR-015).
 *
 * # Why `getDisplayMedia` and not the canvas
 *
 * The play field is a WebGL canvas that Bevy/winit inserts itself, and nothing
 * configures its context — so `preserveDrawingBuffer` is at its default of
 * `false` and `canvas.toDataURL()` outside the drawing frame returns a blank
 * image. This is already recorded twice in the repository
 * (`apps/web/e2e/canvas-authoring.spec.ts` and
 * `apps/engine-sandbox/src/main.ts`), which is why the only `toDataURL` calls
 * here are in Playwright fixtures. A screenshot built that way would produce
 * an empty rectangle where the map was: the failure mode that looks like
 * success. research.md § R7.
 *
 * Screen capture also composites **what the person can actually see** — the
 * canvas, the React panels above it, whatever else they choose to share —
 * which is what FR-015 asks for, and it works on the screens with no engine
 * at all.
 *
 * # The picker is the consent step
 *
 * FR-008 requires that taking a screenshot is the person's choice. The
 * browser's own picker is a native, non-spoofable dialog in which they choose
 * *what* to share — a stronger form of consent than any checkbox this product
 * could draw, and it cannot be pre-answered by the page.
 *
 * # Failure is not an error
 *
 * A refusal, a dismissal, a browser without the API, a frame that will not
 * draw: all of them resolve to `null`. The caller says a plain line and
 * submission proceeds. spec.md's Edge Cases: "offering fails; submitting does
 * not."
 *
 * # The track is stopped in a `finally`
 *
 * Always, on every path including a throw. A sharing indicator that outlives
 * the capture would tell the person their screen is still being watched, and
 * they would be right to believe it.
 */

// The payload's encoder, deliberately: the string the review renders as a
// data URL and the string the payload carries came out of one function.
import { encodeBase64 } from "./feedbackPayload";

/**
 * A captured frame.
 *
 * `pngBase64` is the single representation: the review renders it as a data
 * URL and the payload sends the same string. There is no second encode
 * between what was inspected and what is sent, because a second encode of the
 * same picture is still different bytes.
 */
export interface FeedbackScreenshot {
  pngBase64: string;
  width: number;
  height: number;
  /** Decoded size, for the review to state plainly. */
  byteSize: number;
}

/** Why an offer produced nothing, in words the person can act on. */
export type ScreenshotOutcome =
  | { status: "captured"; screenshot: FeedbackScreenshot }
  | { status: "declined" }
  | { status: "unavailable"; reason: string };

/** Draw one frame from a live track and encode it as a PNG. */
async function encodeFrame(
  track: MediaStreamTrack,
  stream: MediaStream,
): Promise<FeedbackScreenshot | null> {
  const bitmap = await grabFrame(track, stream);
  if (!bitmap) {
    return null;
  }

  try {
    const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
    const context = canvas.getContext("2d");
    if (!context) {
      return null;
    }

    context.drawImage(bitmap, 0, 0);
    const blob = await canvas.convertToBlob({ type: "image/png" });
    const bytes = new Uint8Array(await blob.arrayBuffer());

    return {
      pngBase64: encodeBase64(bytes),
      width: bitmap.width,
      height: bitmap.height,
      byteSize: bytes.length,
    };
  } finally {
    bitmap.close();
  }
}

/**
 * One frame, by whichever route the browser offers.
 *
 * `ImageCapture` is the direct one and is not everywhere; the video-element
 * route works wherever media capture does, which is the point of having it.
 */
async function grabFrame(
  track: MediaStreamTrack,
  stream: MediaStream,
): Promise<ImageBitmap | null> {
  const capture = (
    globalThis as {
      ImageCapture?: new (track: MediaStreamTrack) => {
        grabFrame(): Promise<ImageBitmap>;
      };
    }
  ).ImageCapture;

  if (capture) {
    try {
      return await new capture(track).grabFrame();
    } catch {
      // Fall through: some platforms expose the constructor and refuse the
      // call for a display surface.
    }
  }

  const video = document.createElement("video");
  video.srcObject = stream;
  video.muted = true;

  try {
    await video.play();
    // One frame has to have been decoded before the bitmap exists; `play()`
    // resolving does not guarantee it on every platform.
    await new Promise((resolve) => requestAnimationFrame(resolve));
    return await createImageBitmap(video);
  } catch {
    return null;
  } finally {
    video.pause();
    video.srcObject = null;
  }
}

/**
 * Offer a screenshot.
 *
 * Resolves `declined` when the person dismisses the picker — which is a
 * complete, correct answer and costs them nothing — and `unavailable` with a
 * plain reason when the browser cannot do it at all.
 */
export async function captureScreenshot(): Promise<ScreenshotOutcome> {
  const media = navigator.mediaDevices as MediaDevices | undefined;
  if (!media || typeof media.getDisplayMedia !== "function") {
    return {
      status: "unavailable",
      reason: "This browser cannot capture the screen.",
    };
  }

  if (typeof OffscreenCanvas === "undefined") {
    return {
      status: "unavailable",
      reason: "This browser cannot encode a captured frame.",
    };
  }

  let stream: MediaStream | null = null;
  try {
    stream = await media.getDisplayMedia({ video: true, audio: false });
    const track = stream.getVideoTracks()[0];
    if (!track) {
      return {
        status: "unavailable",
        reason: "The browser shared no video track.",
      };
    }

    const screenshot = await encodeFrame(track, stream);
    return screenshot
      ? { status: "captured", screenshot }
      : {
          status: "unavailable",
          reason: "The captured frame could not be read.",
        };
  } catch (error) {
    // `NotAllowedError` is a person saying no, and is not a failure to report.
    if (error instanceof Error && error.name === "NotAllowedError") {
      return { status: "declined" };
    }
    return {
      status: "unavailable",
      reason: "The screenshot could not be taken.",
    };
  } finally {
    // Every path, including the throw above: the sharing indicator must never
    // outlive the capture.
    for (const track of stream?.getTracks() ?? []) {
      track.stop();
    }
  }
}
