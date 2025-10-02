import type { MiddlewareHandler } from 'astro'

// In dev, normalize loopback host to 127.0.0.1 so cookies
// set by the server with Domain=127.0.0.1 are visible to the app.
export const onRequest: MiddlewareHandler = async ({ request }, next) => {
  if (import.meta.env.DEV) {
    try {
      const url = new URL(request.url)
      if (url.hostname === 'localhost') {
        url.hostname = '127.0.0.1'
        return Response.redirect(url.toString(), 301)
      }
    } catch {}
  }
  return next()
}

