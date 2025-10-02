import type { APIRoute } from 'astro'

export const GET: APIRoute = async ({ request }) => {
  const cookies = request.headers.get('cookie') || ''
  return new Response(
    JSON.stringify({ cookies }, null, 2),
    { status: 200, headers: { 'content-type': 'application/json' } }
  )
}

