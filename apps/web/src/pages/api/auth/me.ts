import type { APIRoute } from 'astro'

const DEFAULT_GRAPHQL = 'http://localhost:8000/graphql'

export const GET: APIRoute = async ({ request }) => {
  const env = import.meta.env as Record<string, string | undefined>
  const graphql = env.PUBLIC_FORGE_GRAPHQL_URL ?? DEFAULT_GRAPHQL
  let base = graphql.replace(/\/graphql$/, '')
  try {
    const u = new URL(base)
    if (u.hostname === 'localhost') {
      u.hostname = '127.0.0.1'
      base = u.toString().replace(/\/$/, '')
    }
  } catch {}
  const sanitizedBase = base.endsWith('/') ? base.slice(0, -1) : base

  const cookie = request.headers.get('cookie') || ''
  const headers = new Headers({ Cookie: cookie })
  try {
    const res = await fetch(`${sanitizedBase}/auth/me`, { headers })
    const body = await res.text()
    return new Response(body, {
      status: res.status,
      headers: { 'content-type': res.headers.get('content-type') || 'application/json' },
    })
  } catch (e) {
    return new Response(JSON.stringify({ authenticated: false, error: 'proxy_failed' }), {
      status: 200,
      headers: { 'content-type': 'application/json' },
    })
  }
}

