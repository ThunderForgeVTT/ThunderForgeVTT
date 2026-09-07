import type { TwoFactorQrMatrix } from "@/types/twoFactor";

export interface TwoFactorQrCodeProps {
  matrix: TwoFactorQrMatrix;
  /** Announced to screen readers in place of the image. */
  label?: string;
}

/** Light modules either side of the code, as the QR spec requires to scan. */
const QUIET_ZONE_MODULES = 4;

/**
 * Draw a QR code from a module matrix (research.md § R10).
 *
 * The server encodes; the client draws rectangles. That decision is the whole
 * point of this component and it buys two things:
 *
 *   - **No `dangerouslySetInnerHTML` on a security screen.** A server-rendered
 *     SVG *string* is markup, and markup from a response is exactly what that
 *     API is for. A matrix of `0`/`1` is data, and cannot become an element.
 *   - **No QR dependency in the bundle.** Reed–Solomon over GF(256) is not
 *     something to hand-roll, but it is also not something the *client* has to
 *     do at all if the server has already done it.
 *
 * One `<rect>` per dark module is more nodes than a path, and for a version-4
 * code (33×33) that is a few hundred rects — well inside what a browser draws
 * without noticing, and much easier to be obviously correct about.
 *
 * Nothing renders this today: `two_factor_setup_start` does not send a `qr`
 * field yet (`src/server/src/auth/two_factor.rs`). It is reached the moment
 * one arrives, which is why `TwoFactorEnrolmentPanel` asks for it
 * conditionally rather than assuming either answer.
 */
export function TwoFactorQrCode({
  matrix,
  label = "QR code for your authenticator app",
}: TwoFactorQrCodeProps) {
  const extent = matrix.size + QUIET_ZONE_MODULES * 2;

  return (
    <svg
      role="img"
      aria-label={label}
      viewBox={`0 0 ${extent} ${extent}`}
      width={extent * 6}
      height={extent * 6}
      shapeRendering="crispEdges"
      className="h-auto w-full max-w-[220px] rounded-md border border-border"
    >
      {/*
        Painted explicitly rather than left transparent: a QR code needs light
        modules to be light, and this panel is legible in both themes, where
        "whatever is behind it" is not a colour a scanner can rely on.
      */}
      <rect x={0} y={0} width={extent} height={extent} fill="#ffffff" />
      {matrix.modules.map((row, rowIndex) =>
        Array.from(row).map((module, columnIndex) =>
          module === "1" ? (
            <rect
              key={`${rowIndex}-${columnIndex}`}
              x={columnIndex + QUIET_ZONE_MODULES}
              y={rowIndex + QUIET_ZONE_MODULES}
              width={1}
              height={1}
              fill="#000000"
            />
          ) : null,
        ),
      )}
    </svg>
  );
}
