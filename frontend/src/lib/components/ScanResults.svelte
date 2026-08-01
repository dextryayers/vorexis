<script lang="ts">
	import FindingsView from '$lib/components/FindingsView.svelte';
	import Markdown from '$lib/components/Markdown.svelte';
	import { api } from '$lib/api/client';
	import { pushToast } from '$lib/stores/app.svelte';
	import { onMount } from 'svelte';
	import type { Scan, ScanEvent } from '$lib/types';

	let { scan, onBack }: { scan: Scan; onBack: () => void } = $props();

	let events = $state<ScanEvent[]>([]);
	let loading = $state(true);
	let report = $state<string | null>(null);
	let reportLoading = $state(false);
	let activeModule = $state<string | null>(null);

	const grouped = $derived.by(() => {
		const map = new Map<string, ScanEvent[]>();
		for (const ev of events) {
			if (ev.event_type !== 'result') continue;
			const list = map.get(ev.module) ?? [];
			list.push(ev);
			map.set(ev.module, list);
		}
		return map;
	});

	const modules = $derived([...grouped.keys()]);

	$effect(() => {
		activeModule = modules[0] ?? null;
	});

	async function load() {
		loading = true;
		try {
			const evs = await api<ScanEvent[]>(`/api/scans/${scan.id}/events?limit=5000`);
			events = evs;
		} catch (e) {
			pushToast(e instanceof Error ? e.message : String(e), 'error');
		} finally {
			loading = false;
		}
	}

	async function generateReport() {
		if (reportLoading) return;
		reportLoading = true;
		report = null;
		try {
			const res = await api<{ report: string }>(`/api/scans/${scan.id}/report`, { method: 'POST' });
			report = res.report ?? 'No report returned.';
		} catch (e) {
			pushToast(e instanceof Error ? e.message : String(e), 'error');
		} finally {
			reportLoading = false;
		}
	}

	onMount(load);
</script>

<div class="flex min-h-0 flex-1 flex-col overflow-y-auto">
	<div class="mx-auto w-full max-w-5xl px-4 py-6">
		<!-- Header -->
		<div class="mb-5 rounded-2xl border border-zinc-800 bg-zinc-900/60 p-4">
			<div class="flex flex-wrap items-center justify-between gap-3">
				<div class="flex min-w-0 items-center gap-2">
					<button
						onclick={onBack}
						title="Back to chat"
						class="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-zinc-800 text-zinc-400 transition hover:border-zinc-700 hover:text-zinc-200"
					>
						<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<path d="M19 12H5M12 19l-7-7 7-7" />
						</svg>
					</button>
					<div class="min-w-0">
						<div class="truncate font-mono text-sm font-semibold text-zinc-100">{scan.target}</div>
						<div class="mt-0.5 flex flex-wrap items-center gap-2 text-[11px] text-zinc-500">
							<span
								class="rounded px-1.5 py-0.5 font-medium {scan.status === 'finished'
									? 'bg-accent-500/10 text-accent-300'
									: scan.status === 'running'
										? 'animate-pulse bg-amber-500/10 text-amber-300'
										: scan.status === 'stopped'
											? 'bg-zinc-800 text-zinc-400'
											: 'bg-red-500/10 text-red-300'}"
							>
								{scan.status}
							</span>
							<span>{scan.modules?.length ?? 0} modules</span>
							<span>{scan.summary?.total_events ?? events.length} events</span>
							{#if scan.error}
								<span class="text-red-400">error: {scan.error}</span>
							{/if}
						</div>
					</div>
				</div>
				<button
					onclick={generateReport}
					disabled={reportLoading || scan.status !== 'finished'}
					class="flex items-center gap-2 rounded-xl border border-accent-500/40 bg-accent-500/10 px-3 py-2 text-xs font-medium text-accent-300 transition enabled:hover:bg-accent-500/20 disabled:opacity-40"
				>
					{#if reportLoading}
						<span class="h-3.5 w-3.5 animate-spin rounded-full border-2 border-accent-300/30 border-t-accent-300"></span>
						Generating...
					{:else}
						<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" />
							<path d="M14 2v6h6M16 13H8M16 17H8M10 9H8" />
						</svg>
						AI Report
					{/if}
				</button>
			</div>

			{#if report}
				<div class="prose-ai mt-4 max-h-96 overflow-y-auto rounded-xl border border-zinc-800 bg-zinc-950/60 p-4">
					<div class="mb-2 flex items-center justify-between">
						<span class="text-[10px] font-semibold uppercase tracking-widest text-accent-400">AI Report</span>
						<button
							onclick={() => (report = null)}
							class="text-[10px] text-zinc-500 hover:text-zinc-300"
						>
							Hide
						</button>
					</div>
					<Markdown source={report} />
				</div>
			{/if}
		</div>

		{#if loading}
			<div class="space-y-3">
				{#each [1, 2, 3] as _}
					<div class="h-24 animate-pulse rounded-xl border border-zinc-800 bg-zinc-900/40"></div>
				{/each}
			</div>
		{:else if modules.length === 0}
			<div class="rounded-xl border border-zinc-800 bg-zinc-900/40 p-8 text-center text-sm text-zinc-500">
				No findings recorded for this scan.
			</div>
		{:else}
			<!-- Module tabs -->
			<div class="mb-4 flex flex-wrap gap-1.5">
				{#each modules as m}
					<button
						onclick={() => (activeModule = m)}
						class="rounded-lg border px-2.5 py-1 font-mono text-[11px] transition {activeModule === m
							? 'border-accent-500/50 bg-accent-500/15 text-accent-300'
							: 'border-zinc-800 bg-zinc-900/60 text-zinc-500 hover:border-zinc-700 hover:text-zinc-300'}"
					>
						{m}
						<span class="ml-1 text-[9px] text-zinc-600">{grouped.get(m)?.length ?? 0}</span>
					</button>
				{/each}
			</div>

			<!-- Module content -->
			<div class="space-y-3">
				{#each (grouped.get(activeModule ?? '') ?? []) as ev, i}
					<div class="overflow-hidden rounded-xl border border-zinc-800 bg-zinc-900/50">
						<div class="flex items-center justify-between border-b border-zinc-800/70 px-3 py-1.5">
							<span class="flex items-center gap-2 font-mono text-[11px] font-semibold text-accent-400">
								<svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
									<path d="M20 6L9 17l-5-5" />
								</svg>
								{ev.module} · #{i + 1}
							</span>
							<span class="font-mono text-[10px] text-zinc-600">{ev.event_type}</span>
						</div>
						<div class="px-3 py-2">
							<FindingsView module={ev.module} data={ev.data} />
						</div>
					</div>
				{/each}
			</div>
		{/if}
	</div>
</div>
