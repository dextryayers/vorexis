<script lang="ts">
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import ScanLive from '$lib/components/ScanLive.svelte';
	import ScanResults from '$lib/components/ScanResults.svelte';
	import { api } from '$lib/api/client';
	import { loadScans, setActiveScan } from '$lib/stores/app.svelte';
	import { onMount } from 'svelte';
	import type { Chat, Scan, ScanEvent } from '$lib/types';

	const scanId = $derived(page.params.id);

	let scan: Scan | null = $state(null);
	let notFound = $state(false);
	let events = $state<ScanEvent[]>([]);
	let scanning = $state(false);

	async function findOwnerChat(): Promise<string> {
		try {
			const chats = await api<Chat[]>('/api/chat');
			const hit = chats.find((c) => c.scan_id === scanId);
			return hit ? `/c/${hit.id}` : '/';
		} catch {
			return '/';
		}
	}

	function goBack() {
		void findOwnerChat().then(goto);
	}

	async function load() {
		try {
			scan = await api<Scan>(`/api/scans/${scanId}`);
			setActiveScan(scan.id);
			scanning = scan.status === 'running';
			loadScans();
		} catch {
			notFound = true;
		}
	}

	$effect(() => {
		void scanId;
		scan = null;
		notFound = false;
		events = [];
		scanning = false;
		load();
	});

	onMount(() => {
		const t = setInterval(() => {
			if (!scan) return;
			api<Scan>(`/api/scans/${scan.id}`)
				.then((s) => {
					scan = s;
					loadScans();
					if (s.status === 'running' && !scanning) {
						events = [];
						scanning = true;
					}
				})
				.catch(() => {
					/* backend unreachable */
				});
		}, 3000);
		return () => clearInterval(t);
	});
</script>

{#if notFound}
	<div class="flex min-h-0 flex-1 flex-col items-center justify-center gap-3 px-4 text-center">
		<div class="text-sm text-zinc-500">This scan no longer exists.</div>
		<button
			onclick={() => goto('/')}
			class="rounded-xl border border-accent-500/40 bg-accent-500/10 px-4 py-2 text-sm font-medium text-accent-300 transition hover:bg-accent-500/20"
		>
			Go home
		</button>
	</div>
{:else if !scan}
	<div class="flex min-h-0 flex-1 items-center justify-center">
		<div class="h-8 w-8 animate-spin rounded-full border-2 border-zinc-700 border-t-accent-500"></div>
	</div>
{:else if scanning}
	<ScanLive bind:events bind:scanning />
{:else}
	<ScanResults scan={scan} onBack={goBack} />
{/if}
