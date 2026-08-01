<script lang="ts">
	import { page } from '$app/state';
	import '../app.css';
	import Sidebar from '$lib/components/Sidebar.svelte';
	import { auth, toasts, bumpComposerFocus } from '$lib/stores/app.svelte';
	import { onMount } from 'svelte';

	let { children } = $props();

	let sidebarOpen = $state(false);

	onMount(() => {
		if (!auth.token && page.url.pathname !== '/login') {
			window.location.href = '/login';
		}
	});

	// React to auth dropping (401 clears the token) while on a protected page.
	$effect(() => {
		if (!auth.token && page.url.pathname !== '/login') {
			window.location.href = '/login';
		}
	});

	$effect(() => {
		void page.url.pathname;
		sidebarOpen = false;
	});

	onMount(() => {
		const onKey = (e: KeyboardEvent) => {
			if (e.key === '/' && !/^(input|textarea|select)$/i.test((e.target as HTMLElement)?.tagName ?? '')) {
				e.preventDefault();
				bumpComposerFocus();
			}
		};
		window.addEventListener('keydown', onKey);
		return () => window.removeEventListener('keydown', onKey);
	});

	const isLogin = $derived(page.url.pathname.startsWith('/login'));
</script>

{#if isLogin}
	{@render children()}
{:else}
	<div class="flex h-dvh w-full overflow-hidden bg-zinc-950 text-zinc-100">
		<Sidebar mobileOpen={sidebarOpen} onCloseMobile={() => (sidebarOpen = false)} />

		<main class="relative flex min-w-0 flex-1 flex-col">
			<button
				onclick={() => (sidebarOpen = true)}
				class="absolute left-3 top-3 z-20 flex h-9 w-9 items-center justify-center rounded-lg border border-zinc-800 bg-zinc-900/80 text-zinc-300 backdrop-blur md:hidden"
				aria-label="Open sidebar"
			>
				<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
					<path d="M3 6h18M3 12h18M3 18h18" />
				</svg>
			</button>
			{@render children()}
		</main>
	</div>
{/if}

<!-- Toasts -->
<div class="pointer-events-none fixed bottom-4 left-1/2 z-50 flex w-full max-w-sm -translate-x-1/2 flex-col items-center gap-2 px-4">
	{#each toasts as t (t.id)}
		<div
			class="pointer-events-auto w-full rounded-xl border px-4 py-2.5 text-sm shadow-lg backdrop-blur {t.kind === 'success'
				? 'border-accent-500/40 bg-zinc-900/95 text-accent-300'
				: t.kind === 'error'
					? 'border-red-500/40 bg-zinc-900/95 text-red-300'
					: 'border-zinc-700 bg-zinc-900/95 text-zinc-200'}"
		>
			{t.message}
		</div>
	{/each}
</div>
