<script lang="ts">
	import { goto } from '$app/navigation';
	import ScanComposer from '$lib/components/ScanComposer.svelte';
	import { api } from '$lib/api/client';
	import { loadChats, loadScans, pushToast, setActiveScan } from '$lib/stores/app.svelte';
	import { onMount } from 'svelte';
	import type { Scan } from '$lib/types';

	let error = $state<string | null>(null);
	let busy = $state(false);

	onMount(() => {
		loadChats();
		loadScans();
		setActiveScan(null);
	});

	async function handleSend(text: string) {
		if (busy) return;
		busy = true;
		error = null;
		try {
			const created = await api<{ id: string }>('/api/chat', {
				method: 'POST',
				body: { title: text.slice(0, 48), scan_id: null }
			});
			loadChats();
			goto(`/c/${created.id}`);
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			busy = false;
		}
	}

	async function handleScan(target: string, modules: string[], options: Record<string, string>) {
		if (busy) return;
		busy = true;
		error = null;
		try {
			const scan = await api<Scan>('/api/scans', {
				method: 'POST',
				body: { target, modules, options }
			});
			setActiveScan(scan.id);
			try {
				const created = await api<{ id: string }>('/api/chat', {
					method: 'POST',
					body: { title: target.slice(0, 48), scan_id: scan.id }
				});
				loadChats();
				pushToast('Scan started', 'success');
				goto(`/c/${created.id}`);
			} catch {
				loadScans();
				pushToast('Scan started — opening results page', 'info');
				goto(`/scan/${scan.id}`);
			}
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally {
			busy = false;
		}
	}
</script>

<div class="flex min-h-0 flex-1 flex-col">
	<div class="flex min-h-0 flex-1 items-center justify-center px-4">
		<div class="mb-8 flex flex-col items-center text-center">
			<div
				class="mb-5 flex h-16 w-16 items-center justify-center rounded-2xl bg-accent-500/10 text-accent-400 ring-1 ring-accent-500/30"
			>
				<svg width="30" height="30" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8">
					<path d="M12 2l8 3.5v5.1c0 5-3.4 9.6-8 10.9-4.6-1.3-8-5.9-8-10.9V5.5L12 2z" />
					<path d="M9 12l2 2 4-4.5" />
				</svg>
			</div>
			<h1 class="text-2xl font-semibold text-zinc-100">AIPentest</h1>
			<p class="mt-2 max-w-md text-sm leading-relaxed text-zinc-500">
				Scan web targets with the Rust engine, then discuss findings with the AI assistant.
			</p>
		</div>
	</div>
	<ScanComposer onSend={handleSend} onScan={handleScan} onStop={() => {}} {error} />
</div>
