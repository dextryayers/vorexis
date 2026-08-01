<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { apiDelete } from '$lib/api/client';
	import { auth, chats, loadChats, logout, pushToast, scans } from '$lib/stores/app.svelte';
	import { onMount } from 'svelte';
	import type { Scan } from '$lib/types';

	let {
		mobileOpen = false,
		onCloseMobile
	}: {
		mobileOpen?: boolean;
		onCloseMobile?: () => void;
	} = $props();

	let collapsed = $state(false);

	const activeChatId = $derived(page.url.pathname.startsWith('/c/') ? page.url.pathname.slice(3) : null);
	const activeScanId = $derived(page.url.pathname.startsWith('/scan/') ? page.url.pathname.slice(6) : null);

	onMount(() => {
		loadChats();
	});

	function newChat() {
		goto('/');
		onCloseMobile?.();
	}

	async function deleteChat(id: string, e: MouseEvent) {
		e.stopPropagation();
		if (!confirm('Delete this chat?')) return;
		try {
			await apiDelete(`/api/chat/${id}`);
			if (activeChatId === id) goto('/');
			pushToast('Chat deleted', 'success');
			loadChats();
		} catch (err) {
			pushToast(err instanceof Error ? err.message : String(err), 'error');
		}
	}

	function selectChat(id: string) {
		goto(`/c/${id}`);
		onCloseMobile?.();
	}

	function selectScan(s: Scan) {
		goto(`/scan/${s.id}`);
		onCloseMobile?.();
	}

	function fmtDate(iso?: string | null): string {
		if (!iso) return '';
		const d = new Date(iso);
		if (Number.isNaN(d.getTime())) return '';
		const now = new Date();
		const sameDay = d.toDateString() === now.toDateString();
		return sameDay
			? d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
			: d.toLocaleDateString([], { day: '2-digit', month: 'short' });
	}

	function statusColor(s: Scan['status']): string {
		if (s === 'finished') return 'bg-accent-500';
		if (s === 'running') return 'animate-pulse bg-amber-400';
		if (s === 'stopped') return 'bg-zinc-500';
		return 'bg-red-500';
	}
</script>

<!-- Mobile overlay -->
{#if mobileOpen}
	<button
		onclick={onCloseMobile}
		aria-label="Close sidebar"
		class="fixed inset-0 z-30 bg-black/60 md:hidden"
	></button>
{/if}

<aside
	class="fixed inset-y-0 left-0 z-40 flex h-full shrink-0 flex-col border-r border-zinc-800 bg-zinc-900/95 backdrop-blur transition-all duration-200 {collapsed
		? 'w-16'
		: 'w-72'} {mobileOpen ? 'translate-x-0' : '-translate-x-full'} md:static md:translate-x-0"
>
	<!-- Header -->
	<div class="flex items-center gap-3 px-4 py-4">
		<a
			href="/"
			title="AIPentest home"
			aria-label="AIPentest home"
			class="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-accent-500/15 text-accent-400 ring-1 ring-accent-500/30"
		>
			<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M12 2l8 3.5v5.1c0 5-3.4 9.6-8 10.9-4.6-1.3-8-5.9-8-10.9V5.5L12 2z" />
				<path d="M9 12l2 2 4-4.5" />
			</svg>
		</a>
		{#if !collapsed}
			<a href="/" class="min-w-0 flex-1">
				<div class="truncate text-sm font-semibold text-zinc-100">AIPentest</div>
				<div class="truncate font-mono text-[10px] text-zinc-500">AI Web Security Scanner</div>
			</a>
		{/if}
		<button
			onclick={() => (collapsed = !collapsed)}
			title={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
			class="hidden text-zinc-600 transition hover:text-zinc-300 md:block"
			aria-label={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
		>
			{#if collapsed}
				<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M9 18l6-6-6-6" />
				</svg>
			{:else}
				<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M15 18l-6-6 6-6" />
				</svg>
			{/if}
		</button>
		<button onclick={onCloseMobile} class="text-zinc-600 transition hover:text-zinc-300 md:hidden" aria-label="Close sidebar">
			<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
				<path d="M18 6L6 18M6 6l12 12" />
			</svg>
		</button>
	</div>

	<!-- New scan / chat -->
	<div class="px-3 pb-2">
		<button
			onclick={newChat}
			class="flex w-full items-center justify-center gap-2 rounded-xl border border-zinc-700 bg-zinc-800/80 px-3 py-2.5 text-sm font-medium text-zinc-200 transition hover:border-accent-500/50 hover:bg-zinc-800 hover:text-accent-300"
		>
			{#if !collapsed}
				<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2">
					<path d="M12 5v14M5 12h14" />
				</svg>
				New Scan
			{:else}
				<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2">
					<path d="M12 5v14M5 12h14" />
				</svg>
			{/if}
		</button>
	</div>

	<!-- Chats -->
	{#if !collapsed}
		<nav class="mt-2 flex-1 space-y-0.5 overflow-y-auto px-3">
			<div class="px-2 pb-1 text-[10px] font-semibold uppercase tracking-widest text-zinc-600">
				Chats
			</div>
			{#each chats as c (c.id)}
				<div
					class="group flex items-center rounded-lg transition {activeChatId === c.id
						? 'bg-zinc-800 text-zinc-100'
						: 'hover:bg-zinc-800/70 text-zinc-400'}"
				>
					<button
						onclick={() => selectChat(c.id)}
						class="flex min-w-0 flex-1 items-center gap-2 truncate rounded-lg px-2 py-2 text-left text-sm hover:text-zinc-200"
					>
						<span class="inline-block w-3.5 shrink-0">
							<svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
								<path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
							</svg>
						</span>
						<span class="min-w-0 flex-1 truncate align-middle">
							{c.title}
							<span class="ml-1 text-[9px] text-zinc-600">{fmtDate(c.updated_at)}</span>
						</span>
					</button>
					<button
						onclick={(e) => deleteChat(c.id, e)}
						title="Delete chat"
						class="mr-1 shrink-0 rounded p-1 text-zinc-600 opacity-0 transition hover:text-red-400 group-hover:opacity-100"
					>
						<svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
							<path d="M3 6h18M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2m3 0v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6" />
						</svg>
					</button>
				</div>
			{/each}

			<div class="px-2 pb-1 pt-4 text-[10px] font-semibold uppercase tracking-widest text-zinc-600">
				Recent scans
			</div>
			{#each scans.slice(0, 10) as s (s.id)}
				<button
					onclick={() => selectScan(s)}
					class="flex w-full items-center gap-2 rounded-lg px-2 py-1.5 text-left text-xs transition hover:bg-zinc-800/70 {activeScanId === s.id
						? 'bg-zinc-800 text-zinc-200'
						: 'text-zinc-500'}"
				>
					<span class="h-1.5 w-1.5 shrink-0 rounded-full {statusColor(s.status)}"></span>
					<span class="min-w-0 flex-1 truncate font-mono">{s.target}</span>
					<span class="shrink-0 text-[9px] text-zinc-600">{fmtDate(s.finished_at ?? s.started_at)}</span>
				</button>
			{/each}
		</nav>
	{:else}
		<nav class="flex flex-1 flex-col items-center gap-1 pt-2">
			{#each chats as c (c.id)}
				<button
					onclick={() => selectChat(c.id)}
					title={c.title}
					class="h-8 w-8 rounded-lg text-center text-xs font-semibold text-zinc-500 hover:bg-zinc-800"
				>
					{c.title.charAt(0).toUpperCase()}
				</button>
			{/each}
		</nav>
	{/if}

	<!-- Footer -->
	<div class="border-t border-zinc-800/80 p-3">
		{#if !collapsed}
			<div class="flex items-center gap-2.5">
				<div
					class="flex h-8 w-8 items-center justify-center rounded-full bg-gradient-to-br from-accent-500 to-emerald-700 text-xs font-bold text-zinc-950"
				>
					{(auth.username ?? '?').charAt(0).toUpperCase()}
				</div>
				<div class="min-w-0 flex-1">
					<div class="truncate text-xs font-medium text-zinc-300">{auth.username}</div>
					<div class="text-[10px] text-zinc-600">Rust engine · FastAPI</div>
				</div>
				<button onclick={logout} title="Logout" class="text-zinc-500 transition hover:text-red-400">
					<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
						<path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4M16 17l5-5-5-5M21 12H9" />
					</svg>
				</button>
			</div>
		{:else}
			<div class="flex justify-center">
				<button
					onclick={logout}
					title="Logout"
					class="text-zinc-500 transition hover:text-red-400"
				>
					<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
						<path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4M16 17l5-5-5-5M21 12H9" />
					</svg>
				</button>
			</div>
		{/if}
	</div>
</aside>
