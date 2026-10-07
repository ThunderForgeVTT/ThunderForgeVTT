/**
 * The client's one `XMLHttpRequest` is the book importer's upload, with a
 * progress bar. There is nowhere for it to go, so it is answered the way a
 * refused `fetch` is: a 404 carrying the demo's own refusal, delivered as a
 * finished request. The importer reads that and says so in its own words,
 * instead of meeting a constructor that throws.
 */

type Listener = (event: Event) => void;

class Events {
  private listeners = new Map<string, Set<Listener>>();

  addEventListener(type: string, listener: Listener): void {
    if (!this.listeners.has(type)) this.listeners.set(type, new Set());
    this.listeners.get(type)!.add(listener);
  }

  removeEventListener(type: string, listener: Listener): void {
    this.listeners.get(type)?.delete(listener);
  }

  protected fire(type: string): void {
    const event = new Event(type);
    const handler = (this as unknown as Record<string, unknown>)[`on${type}`];
    if (typeof handler === "function") handler.call(this, event);
    for (const listener of this.listeners.get(type) ?? []) listener(event);
  }
}

class RefusedUpload extends Events {
  onprogress: Listener | null = null;
  onload: Listener | null = null;
  onloadend: Listener | null = null;
  onerror: Listener | null = null;
  onabort: Listener | null = null;
  ontimeout: Listener | null = null;
}

/**
 * `refusal` is the body a refused request is answered with; `refuse` is
 * called once the request is sent, to tell the visitor.
 */
export function refusingXmlHttpRequest(
  refusal: () => string,
  refuse: () => void,
): typeof XMLHttpRequest {
  class DemoXmlHttpRequest extends Events {
    static readonly UNSENT = 0;
    static readonly OPENED = 1;
    static readonly HEADERS_RECEIVED = 2;
    static readonly LOADING = 3;
    static readonly DONE = 4;

    readonly upload = new RefusedUpload();
    readyState = 0;
    status = 0;
    statusText = "";
    responseText = "";
    response: unknown = "";
    responseType = "";
    responseURL = "";
    timeout = 0;
    withCredentials = false;
    onreadystatechange: Listener | null = null;
    onload: Listener | null = null;
    onloadend: Listener | null = null;
    onerror: Listener | null = null;
    onabort: Listener | null = null;
    ontimeout: Listener | null = null;
    onprogress: Listener | null = null;
    private aborted = false;

    open(_method: string, url: string | URL): void {
      this.responseURL = String(url);
      this.readyState = 1;
      this.fire("readystatechange");
    }

    setRequestHeader(): void {}

    overrideMimeType(): void {}

    getAllResponseHeaders(): string {
      return this.readyState === 4 ? "content-type: application/json\r\n" : "";
    }

    getResponseHeader(name: string): string | null {
      return this.readyState === 4 && name.toLowerCase() === "content-type"
        ? "application/json"
        : null;
    }

    send(): void {
      // Asynchronously, as a real request finishes: the caller has finished
      // wiring its handlers before any of them is called.
      setTimeout(() => {
        if (this.aborted) return;
        refuse();
        this.status = 404;
        this.statusText = "Not Found";
        this.responseText = refusal();
        this.response = this.responseText;
        this.readyState = 4;
        this.fire("readystatechange");
        this.fire("load");
        this.fire("loadend");
      }, 0);
    }

    abort(): void {
      if (this.readyState === 4 || this.aborted) return;
      this.aborted = true;
      this.readyState = 4;
      this.fire("readystatechange");
      this.fire("abort");
      this.fire("loadend");
    }
  }
  return DemoXmlHttpRequest as unknown as typeof XMLHttpRequest;
}
