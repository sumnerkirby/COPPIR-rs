// Path-based routing: `/` lists exercises, `/x/<id>` opens one. The server
// answers every non-API path with index.html, so any URL can be reloaded or
// shared.

class Router {
  path = $state(location.pathname);

  constructor() {
    addEventListener('popstate', () => (this.path = location.pathname));
  }

  go(path: string) {
    if (path === location.pathname) return;
    history.pushState(null, '', path);
    this.path = path;
  }

  get exerciseId(): string | null {
    return this.path.match(/^\/x\/([\w-]+)/)?.[1] ?? null;
  }
}

export const router = new Router();
