/** A download that cannot finish (FR-019). Nothing partial is delivered. */
export class DownloadError extends Error {
  readonly url: string;
  /** The last HTTP status seen, or `null` for a network failure. */
  readonly status: number | null;

  constructor(
    url: string,
    status: number | null,
    message?: string,
    cause?: unknown,
  ) {
    super(
      message ??
        `download of ${url} failed${status === null ? "" : ` (${status})`}`,
      {
        cause,
      },
    );
    this.name = "DownloadError";
    this.url = url;
    this.status = status;
  }
}

/**
 * The file changed on the server after bytes of the old version had already
 * been handed on (FR-015). The caller starts again; `downloadBytes` does so
 * itself.
 */
export class VersionChangedError extends Error {
  readonly url: string;

  constructor(url: string) {
    super(`${url} changed on the server during the download`);
    this.name = "VersionChangedError";
    this.url = url;
  }
}
