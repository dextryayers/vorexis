<script lang="ts">
	import { onMount } from 'svelte';
	import { api, wsUrl } from '$lib/api/client';
	import { activeScan, loadScans, pushToast } from '$lib/stores/app.svelte';
	import type { Scan, ScanEvent } from '$lib/types';
	import FindingsView from '$lib/components/FindingsView.svelte';

	let { events = $bindable([]), scanning = $bindable(false) }: { events: ScanEvent[]; scanning: boolean } = $props();

	let ws: WebSocket | null = null;
	let progress = $state(0);
	let statusText = $state('Preparing engine...');
	let elapsed = $state(0);
	let failed = $state(false);
	let stopping = $state(false);
	let moduleStats = $state<Record<string, { results: number; events: number }>>({});
	let reconnectAttempts = 0;

	$effect(() => {
		if (!scanning) return;
		const t = setInterval(() => elapsed++, 1000);
		return () => clearInterval(t);
	});

	function fmtElapsed(s: number): string {
		const m = Math.floor(s / 60);
		const sec = s % 60;
		return m > 0 ? `${m}m ${sec}s` : `${sec}s`;
	}

	function connect() {
		if (!activeScan.id) return;
		ws = new WebSocket(wsUrl(activeScan.id));
		ws.onmessage = (msg) => {
			try {
				const ev = JSON.parse(msg.data) as ScanEvent;
				handle(ev);
			} catch {
				/* ignore */
			}
		};
		ws.onclose = () => {
			ws = null;
			// Server may have restarted mid-scan — retry briefly, then fall
			// back to polling.
			if (scanning && reconnectAttempts < 3 && !failed) {
				reconnectAttempts++;
				setTimeout(connect, reconnectAttempts * 1500);
			} else {
				setTimeout(() => loadScans(), 400);
			}
		};
		ws.onerror = () => ws?.close();
	}

	function handle(raw: unknown) {
		if (!raw || typeof raw !== 'object') return;
		const r = raw as Record<string, unknown>;
		// Engine wire format uses `type`/`current`/`total`/`message` at top level;
		// DB events use `event_type`/`data`. Normalize both.
		const etype = String(r.event_type ?? r.type ?? 'event');
		const module = String(r.module ?? 'engine');
		const data = (r.data ?? null) as Record<string, unknown> | null;

		const stats = (moduleStats[module] ??= { results: 0, events: 0 });
		stats.events++;
		if (etype === 'progress') {
			const current = typeof r.current === 'number' ? r.current : (data?.current as number | undefined);
			const total = typeof r.total === 'number' ? r.total : (data?.total as number | undefined);
			if (typeof current === 'number' && typeof total === 'number' && total > 0) {
				progress = Math.max(progress, Math.round((current / total) * 100));
			}
		} else if (etype === 'event') {
			const msg = (r.message ?? data?.message ?? null) as string | null;
			if (msg) statusText = msg;
		} else if (etype === 'result') {
			stats.results++;
			events.push({
				scan_id: String(r.scan_id ?? activeScan.id ?? ''),
				module,
				event_type: 'result',
				data: data ?? r
			});
		} else if (etype === 'complete') {
			const dur = typeof r.duration_ms === 'number' ? r.duration_ms : (data?.duration_ms as number | undefined);
			statusText = `Module ${module} finished (${dur ?? 0}ms)`;
		} else if (etype === 'done') {
			const returncode = typeof r.returncode === 'number' ? r.returncode : (data?.returncode as number | undefined);
			const summary = (data?.summary ?? null) as Record<string, unknown> | null;
			failed = returncode !== undefined && returncode !== 0;
			scanning = false;
			statusText = failed ? 'Scan failed' : 'Scan complete';
			progress = 100;
			if (failed) pushToast('Engine reported an error — see status details', 'error');
			void summary;
			ws?.close();
			setTimeout(() => loadScans(), 300);
		}
	}

	async function stop() {
		if (!activeScan.id || stopping) return;
		stopping = true;
		try {
			await api(`/api/scans/${activeScan.id}/stop`, { method: 'POST' });
			statusText = 'Stopping...';
			pushToast('Stopping scan', 'info');
		} catch (e) {
			pushToast(e instanceof Error ? e.message : String(e), 'error');
		} finally {
			stopping = false;
		}
	}

	onMount(() => {
		connect();
		return () => ws?.close();
	});
</script>

<div class="flex min-h-0 flex-1 flex-col overflow-y-auto">
	<div class="mx-auto w-full max-w-4xl px-4 py-6">
		<!-- Progress header -->
		<div class="mb-5 rounded-2xl border border-zinc-800 bg-zinc-900/60 p-4">
			<div class="mb-2 flex items-center justify-between gap-3">
				<div class="flex items-center gap-2 text-sm font-medium text-zinc-200">
					<span class="relative flex h-2.5 w-2.5">
						<span
							class="absolute inline-flex h-full w-full animate-ping rounded-full {failed ? 'bg-red-500' : 'bg-accent-400'} opacity-60"
						></span>
						<span class="relative inline-flex h-2.5 w-2.5 rounded-full {failed ? 'bg-red-500' : 'bg-accent-500'}"></span>
					</span>
					{failed ? 'Scan failed' : 'Scan in progress'}
				</div>
				<div class="flex items-center gap-3">
					<span class="font-mono text-xs text-zinc-500">{fmtElapsed(elapsed)}</span>
					<span class="font-mono text-xs text-zinc-500">{progress}%</span>
					<button
						onclick={stop}
						disabled={stopping || failed}
						class="flex items-center gap-1.5 rounded-lg border border-red-500/40 bg-red-500/10 px-2.5 py-1.5 text-[11px] font-medium text-red-300 transition enabled:hover:bg-red-500/20 disabled:opacity-40"
					>
						<svg width="10" height="10" viewBox="0 0 24 24" fill="currentColor">
							<rect x="6" y="6" width="12" height="12" rx="2" />
						</svg>
						{stopping ? 'Stopping...' : 'Stop'}
					</button>
				</div>
			</div>
			<div class="h-1.5 w-full overflow-hidden rounded-full bg-zinc-800">
				<div
					class="h-full rounded-full transition-all duration-500 {failed
						? 'bg-gradient-to-r from-red-600 to-red-400'
						: 'bg-gradient-to-r from-accent-600 to-accent-400'}"
					style="width: {progress}%"
				></div>
			</div>
			<div class="mt-2 font-mono text-[11px] text-zinc-500">{statusText}</div>

			<!-- Module chips -->
			<div class="mt-3 flex flex-wrap gap-1.5">
				{#each Object.entries(moduleStats) as [mod, st]}
					<div
						class="rounded-lg border border-zinc-700/70 bg-zinc-800/60 px-2 py-1 font-mono text-[10px] text-zinc-400"
					>
						{mod}
						<span class="text-accent-400">{st.results} results</span>
					</div>
				{/each}
			</div>
		</div>

		<!-- Live findings -->
		{#if events.length > 0}
			<div class="mb-3 text-[10px] font-semibold uppercase tracking-widest text-zinc-600">
				Live findings ({events.length})
			</div>
			<div class="space-y-3">
				{#each events.slice(-30) as ev}
					<div class="overflow-hidden rounded-xl border border-zinc-800 bg-zinc-900/50">
						<div class="flex items-center justify-between border-b border-zinc-800/70 px-3 py-1.5">
							<span class="flex items-center gap-2 font-mono text-[11px] font-semibold text-accent-400">
								<svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
									<path d="M20 6L9 17l-5-5" />
								</svg>
								{ev.module}
							</span>
							<span class="font-mono text-[10px] text-zinc-600">{ev.event_type}</span>
						</div>
						<div class="px-3 py-2">
							<FindingsView module={ev.module} data={ev.data} compact />
						</div>
					</div>
				{/each}
			</div>
		{/if}
	</div>
</div>
