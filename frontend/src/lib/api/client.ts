// Minimal fetch wrapper: attaches JWT, throws on errors, JSON-aware.

export const API_BASE = import.meta.env.PUBLIC_API_BASE ?? '';

export function getToken(): string | null {
	if (typeof localStorage === 'undefined') return null;
	return localStorage.getItem('aipentest_token');
}

export function setToken(token: string | null) {
	if (typeof localStorage === 'undefined') return;
	if (token) localStorage.setItem('aipentest_token', token);
	else localStorage.removeItem('aipentest_token');
}

export function getUsername(): string | null {
	if (typeof localStorage === 'undefined') return null;
	return localStorage.getItem('aipentest_username');
}

export function setUsername(name: string | null) {
	if (typeof localStorage === 'undefined') return;
	if (name) localStorage.setItem('aipentest_username', name);
	else localStorage.removeItem('aipentest_username');
}

export async function api<T = unknown>(
	path: string,
	options: Omit<RequestInit, 'body'> & { body?: unknown } = {}
): Promise<T> {
	const token = getToken();
	const headers: Record<string, string> = {
		...(options.headers as Record<string, string> | undefined)
	};
	if (options.body !== undefined) headers['Content-Type'] = 'application/json';
	if (token) headers['Authorization'] = `Bearer ${token}`;

	const res = await fetch(`${API_BASE}${path}`, {
		...options,
		headers,
		body: options.body !== undefined ? JSON.stringify(options.body) : undefined
	});

	if (!res.ok) {
		let detail = res.statusText;
		try {
			const data = await res.json();
			detail = data.detail ?? JSON.stringify(data);
		} catch {
			/* not json */
		}
		if (res.status === 401) {
			setToken(null);
		}
		throw new Error(detail);
	}
	if (res.status === 204) return undefined as T;
	return (await res.json()) as T;
}

export function wsUrl(scanId: string): string {
	const base = API_BASE || window.location.origin;
	const wsBase = base.replace(/^http/, 'ws');
	return `${wsBase}/ws/scan/${scanId}?token=${encodeURIComponent(getToken() ?? '')}`;
}

/**
 * Stream an SSE endpoint line by line. Yields the `data:` payload of every
 * event (parsed as JSON when possible). Resolves when the stream ends.
 * Returns an object with `stream`, `stop()` (aborts), and `done` promise.
 */
export function streamSSE(
	path: string,
	body: unknown,
	signal?: AbortSignal
): { stream: ReadableStream<unknown>; stop: () => void } {
	const controller = new AbortController();
	const onOuterAbort = () => controller.abort();
	if (signal) {
		if (signal.aborted) controller.abort();
		else signal.addEventListener('abort', onOuterAbort);
	}

	const token = getToken();
	const headers: Record<string, string> = { 'Content-Type': 'application/json' };
	if (token) headers['Authorization'] = `Bearer ${token}`;

	const stop = () => {
		signal?.removeEventListener('abort', onOuterAbort);
		controller.abort();
	};

	const stream = new ReadableStream<unknown>({
		async start(controllerSink) {
			try {
				const res = await fetch(`${API_BASE}${path}`, {
					method: 'POST',
					headers,
					body: JSON.stringify(body),
					signal: controller.signal
				});
				if (!res.ok || !res.body) {
					let detail = res.statusText;
					try {
						const data = await res.json();
						detail = data.detail ?? JSON.stringify(data);
					} catch {
						/* not json */
					}
					if (res.status === 401) setToken(null);
					controllerSink.error(new Error(detail));
					return;
				}
				const reader = res.body.getReader();
				const decoder = new TextDecoder();
				let buffer = '';
				for (;;) {
					const { done, value } = await reader.read();
					if (done) break;
					buffer += decoder.decode(value, { stream: true });
					let nl: number;
					while ((nl = buffer.indexOf('\n')) >= 0) {
						const line = buffer.slice(0, nl).trim();
						buffer = buffer.slice(nl + 1);
						if (!line.startsWith('data:')) continue;
						const payload = line.slice(5).trim();
						if (!payload) continue;
						let parsed: unknown = payload;
						try {
							parsed = JSON.parse(payload);
						} catch {
							/* keep raw string */
						}
						controllerSink.enqueue(parsed);
					}
				}
				controllerSink.close();
			} catch (e) {
				controllerSink.error(e);
			}
		},
		cancel() {
			stop();
		}
	});

	return { stream, stop };
}

export async function apiDelete(path: string): Promise<void> {
	const token = getToken();
	const headers: Record<string, string> = {};
	if (token) headers['Authorization'] = `Bearer ${token}`;
	const res = await fetch(`${API_BASE}${path}`, { method: 'DELETE', headers });
	if (!res.ok && res.status !== 204) {
		let detail = res.statusText;
		try {
			const data = await res.json();
			detail = data.detail ?? JSON.stringify(data);
		} catch {
			/* not json */
		}
		if (res.status === 401) setToken(null);
		throw new Error(detail);
	}
}
